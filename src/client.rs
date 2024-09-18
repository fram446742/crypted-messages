use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
    ExecutableCommand,
};
use std::sync::Arc;
use std::{collections::VecDeque, str};
use std::{error::Error as StdError, time::Instant};
use std::{io::Write, process};
use tokio::net::TcpStream;
use tokio::spawn;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{self, timeout, Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    time::sleep,
};

use crate::tools::{
    decrypt_handshake, decrypt_message, encrypt_handshake, encrypt_message, get_ip, get_port,
    get_timestamp, AdressMode, ClientCommand, Handshake, Message, SerdeColor,
};

type Instance = Arc<Mutex<(String, SerdeColor)>>;
type Key = Arc<String>;
type ColorBool = Arc<Mutex<bool>>;
const BUFFER_SIZE: usize = 1024;
const CHUNK_SIZE: usize = 1024; // Define your chunk size
const CHUNKED_SIGNAL: &str = "START_CHUNK";
const FINAL_CHUNK_SIGNAL: &str = "END_CHUNK";
const CLOSE_SIGNAL: &str = "CLOSE_CONNECTION";
const COLOR_CHANGE_SIGNAL: &str = "COLOR_CHANGE";
// const NAME_CHANGE_SIGNAL: &str = "NAME_CHANGE";
const CONNECTION_TIMEOUT: u64 = 30;
const RETRY_DELAY: u64 = 3; // Delay between connection attempts (in seconds)
const HELP_MESSAGE: &str = "
Commands:
/toggle-color - Toggle color mode
/change-color to change your color
/view-messages to view your messages.
/help - Show this help message
/sudo (password) - Be granted admin privileges
/close - Close gracefully the connection
/quit - Forcefully quit the application
";

pub async fn main_client() -> Result<(), Box<dyn StdError + Send + Sync>> {
    let server_ip = get_ip(None, None, AdressMode::Client)?;
    let server_port = get_port(None, None, AdressMode::Client)?;

    // Keep trying to connect to the server with a 30-second timeout
    let socket = wait_for_server(&server_ip, server_port).await?;

    let (reader, mut writer) = socket.into_split();
    let (tx, rx) = mpsc::unbounded_channel();
    let mut reader = BufReader::new(reader);

    // Set key and instance (name + color)
    let key: Key = set_key().await?;
    let instance: Instance = set_name().await?;
    let color_bool = Arc::new(Mutex::new(true));
    let color_bool_clone = color_bool.clone();

    let mut buffer = [0u8; BUFFER_SIZE];

    // Send initial handshake to server
    send_initial_handshake(&key, &instance, &mut writer).await?;

    // Task to handle input from stdin and send to the server
    let tx_clone = tx.clone();
    spawn(async move {
        handle_stdin_input(tx_clone).await.unwrap_or_else(|e| {
            eprintln!("Error handling stdin input: {:?}", e);
        });
    });

    // Read handshake response from the server with timeout
    if let Err(_) = timeout(
        Duration::from_secs(10),
        handle_handshake_response(&key, &instance, &mut reader, &mut buffer),
    )
    .await
    {
        eprintln!("Server handshake timed out");
        return Ok(());
    }

    let key_clone = key.clone();

    let mut chunk_buffer = VecDeque::new();

    let instance_clone = instance.clone();

    // Task to handle incoming server messages
    spawn(async move {
        if let Err(e) = handle_incoming_messages(
            key_clone,
            &mut reader,
            &mut chunk_buffer,
            color_bool_clone,
            &instance_clone,
        )
        .await
        {
            eprintln!("Error handling incoming messages: {:?}", e);
        }
    });

    // Sending messages to the server
    send_messages_to_server(rx, &key, &instance, &mut writer, color_bool).await?;

    Ok(())
}

async fn toogle_color(color_bool: ColorBool) {
    let mut color_bool = color_bool.lock().await;
    *color_bool = !*color_bool;
    println!("Color mode toogled");
}

// Function to send the initial handshake to the server
async fn send_initial_handshake(
    key: &Key,
    instance: &Instance,
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
) -> Result<(), Box<dyn StdError + Send + Sync>> {
    let handshake = Handshake::new(instance.lock().await.0.clone(), BUFFER_SIZE, None);
    let encrypted_handshake = encrypt_handshake(&key, &handshake)?;

    writer.write_all(&encrypted_handshake).await?;
    writer.flush().await?;

    Ok(())
}

// Handle handshake response from the server
async fn handle_handshake_response(
    key: &Key,
    instance: &Instance,
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    buffer: &mut [u8],
) -> Result<(), Box<dyn StdError + Send + Sync>> {
    match reader.read(buffer).await {
        Ok(0) => {
            eprintln!("Server disconnected during handshake");
            return Ok(());
        }
        Ok(n) => {
            let handshake = decrypt_handshake(&key, &buffer[..n])?;
            if handshake.name == instance.lock().await.0 {
                println!("Handshake successful");
                instance.lock().await.1 = handshake.color.unwrap_or(SerdeColor::Red);
            } else {
                eprintln!("Handshake name mismatch");
                println!("Updating name: {}", handshake.name);
                instance.lock().await.0 = handshake.name;
                instance.lock().await.1 = handshake.color.unwrap_or(SerdeColor::Red);
                return Ok(());
            }
        }
        Err(e) => {
            eprintln!("Failed to read handshake response from server: {:?}", e);
            return Ok(());
        }
    }
    Ok(())
}

// Task to handle reading from stdin and sending chunked messages to a channel
async fn handle_stdin_input(
    tx: mpsc::UnboundedSender<String>,
) -> Result<(), Box<dyn StdError + Send + Sync>> {
    let mut stdin = BufReader::new(tokio::io::stdin()).take(usize::MAX as u64);
    let mut buffer = Vec::new();

    loop {
        buffer.clear();
        let bytes_read = stdin.read_buf(&mut buffer).await?;

        if bytes_read == 0 {
            // End of input
            break;
        }

        if buffer.len() > CHUNK_SIZE {
            println!("Sending chunked message to server");

            // Notify that we are starting to send chunks
            if tx.send(CHUNKED_SIGNAL.to_string()).is_err() {
                eprintln!("Failed to send chunked signal");
                return Err("Failed to send chunked signal".into());
            }

            // Introduce delay after sending the chunked signal
            time::sleep(Duration::from_millis(10)).await;

            // Split the buffer into chunks and send each
            for chunk in buffer.chunks(CHUNK_SIZE) {
                let chunked_line = match str::from_utf8(chunk) {
                    Ok(valid_str) => valid_str.trim().to_string(), // Trim \r\n
                    Err(_) => String::from_utf8_lossy(chunk).trim().to_string(), // Trim \r\n
                };

                // Send each chunk
                if tx.send(chunked_line).is_err() {
                    eprintln!("Failed to send message chunk from stdin");
                    return Err("Failed to send message chunk".into());
                }

                // Introduce a delay between sending chunks to prevent server overload
                time::sleep(Duration::from_millis(10)).await;
            }

            // Send final chunk signal
            if tx.send(FINAL_CHUNK_SIGNAL.to_string()).is_err() {
                eprintln!("Failed to send final chunk signal");
                return Err("Failed to send final chunk signal".into());
            }

            // Introduce delay after sending the final chunk signal
            time::sleep(Duration::from_millis(10)).await;
        } else {
            // Send the message as a whole if it's smaller than CHUNK_SIZE
            let message = match str::from_utf8(&buffer) {
                Ok(valid_str) => valid_str.trim().to_string(), // Trim \r\n
                Err(_) => String::from_utf8_lossy(&buffer).trim().to_string(), // Trim \r\n
            };

            // Send the entire message
            if tx.send(message).is_err() {
                eprintln!("Failed to send message from stdin");
                return Err("Failed to send message".into());
            }

            // Introduce delay after sending the whole message
            time::sleep(Duration::from_millis(10)).await;
        }
    }
    Ok(())
}

// Task to handle incoming messages from the server
async fn handle_incoming_messages(
    key: Key,
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    chunk_buffer: &mut VecDeque<String>,
    color_bool: ColorBool,
    instance: &Instance,
) -> Result<(), Box<dyn StdError + Send + Sync>> {
    let mut buffer = [0u8; BUFFER_SIZE];
    // let mut chunk_buffer = VecDeque::new();
    let mut is_chunked_message = false;

    loop {
        let color_bool = color_bool.clone();
        match reader.read(&mut buffer).await {
            Ok(n) if n == 0 => {
                // When n is 0, the server has closed the connection gracefully
                eprintln!("Server disconnected gracefully");
                return Err("Server disconnected".into());
            }
            Ok(n) => {
                // Handle the message if the server is still sending data
                let decrypted_msg = decrypt_message(&key, &buffer[..n])?;

                if let Some(message) = decrypted_msg.message {
                    match message.as_str() {
                        CHUNKED_SIGNAL => {
                            // Start of a chunked message
                            println!("Starting to accumulate chunked messages");
                            is_chunked_message = true;
                        }
                        FINAL_CHUNK_SIGNAL => {
                            // End of a chunked message, process the accumulated chunks
                            let complete_message = chunk_buffer.iter().cloned().collect::<String>();
                            chunk_buffer.clear(); // Clear the buffer

                            let _ = print_colored_text(
                                &format!(
                                    "{}: {}",
                                    decrypted_msg
                                        .name
                                        .unwrap_or_else(|| "Unknown sender".to_string()),
                                    complete_message
                                ),
                                Color::from(decrypted_msg.color.unwrap_or(SerdeColor::Red)),
                                color_bool,
                            )
                            .await;

                            is_chunked_message = false;
                        }
                        CLOSE_SIGNAL => {
                            // Server has closed the connection
                            println!("Server has closed the connection");
                            // Wait for an input to exit the client
                            for i in (1..=3).rev() {
                                print!("\rClosing in {}...", i);
                                std::io::stdout().flush().unwrap();
                                time::sleep(Duration::from_secs(1)).await;
                            }
                            std::io::stdout().flush().unwrap();
                            print!("\rClosed...         ");
                            // time::sleep(Duration::from_secs(1)).await;
                            // FEATURE: Add restart option
                            process::exit(0);
                        }
                        COLOR_CHANGE_SIGNAL => {
                            // Change the color of the client
                            let new_color = decrypted_msg.color.unwrap_or(SerdeColor::Red);
                            instance.lock().await.1 = new_color;
                            let _ = print_colored_text(
                                &format!("Color changed to {:?}", new_color),
                                Color::from(new_color),
                                color_bool,
                            );
                        }
                        // NAME_CHANGE_SIGNAL => {
                        //     // Change the name of the client
                        //     let new_name = decrypted_msg
                        //         .name
                        //         .unwrap_or_else(|| "Unknown sender".to_string());
                        //     instance.lock().await.0 = new_name.clone();
                        //     let _ = print_colored_text(
                        //         &format!("Name changed to {:?}", new_name),
                        //         Color::from(decrypted_msg.color.unwrap_or(SerdeColor::Red)),
                        //         color_bool,
                        //     );
                        // }
                        _ => {
                            if is_chunked_message {
                                // Accumulate message as part of a chunked message
                                chunk_buffer.push_back(message);
                            } else {
                                // Display regular individual message
                                let _ = print_colored_text(
                                    &format!(
                                        "{}: {}",
                                        decrypted_msg
                                            .name
                                            .unwrap_or_else(|| "Unknown sender".to_string()),
                                        message
                                    ),
                                    Color::from(decrypted_msg.color.unwrap_or(SerdeColor::Red)),
                                    color_bool,
                                )
                                .await;
                            }
                        }
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {
                // Handle when the connection is reset (e.g., server crashes or forcefully disconnects)
                eprintln!("Server connection reset: {:?}", e);
                return Err("Server connection reset".into());
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Handle a timeout if there is one set up for the reader
                eprintln!("Server connection timed out: {:?}", e);
                return Err("Server connection timed out".into());
            }
            Err(e) => {
                // Handle other types of errors, like I/O errors
                eprintln!("Error while reading from server: {:?}", e);
                return Err(e.into());
            }
        }
    }
}

// Task to send messages to the server
async fn send_messages_to_server(
    mut rx: mpsc::UnboundedReceiver<String>,
    key: &Key,
    instance: &Instance,
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    color_bool: ColorBool,
) -> Result<(), Box<dyn StdError + Send + Sync>> {
    while let Some(line) = rx.recv().await {
        let instance_lock = instance.lock().await;
        let color = Color::from(instance_lock.1);
        let color_bool = color_bool.clone();

        // println!("You: {}", line);

        match ClientCommand::from_str(&line) {
            ClientCommand::ToogleColor => {
                toogle_color(color_bool).await;
            }
            ClientCommand::Help => {
                let _ = print_colored_text(HELP_MESSAGE, color, color_bool).await;
            }
            ClientCommand::Quit => {
                let _ = print_colored_text("Forcefully quitting...", color, color_bool).await;
                sleep(Duration::from_secs(1)).await;
                process::exit(0);
            }
            _ => {
                match line.as_str() {
                    CHUNKED_SIGNAL => {
                        let chunk_message = Message::new(
                            Some(instance_lock.0.clone()),
                            Some(get_timestamp()),
                            Some("CHUNK_MESSAGE".to_string()),
                            Some(instance_lock.1),
                        );

                        let encrypted_chunk_message = encrypt_message(&key, &chunk_message)?;

                        if writer.write_all(&encrypted_chunk_message).await.is_err() {
                            eprintln!("Failed to send chunked message to server");
                            break;
                        }
                    }
                    FINAL_CHUNK_SIGNAL => {
                        let final_chunk_signal = Message::new(
                            Some(instance_lock.0.clone()),
                            Some(get_timestamp()),
                            Some("END_CHUNK".to_string()),
                            Some(instance_lock.1),
                        );

                        let encrypted_final_chunk = encrypt_message(&key, &final_chunk_signal)?;

                        if writer.write_all(&encrypted_final_chunk).await.is_err() {
                            eprintln!("Failed to send final chunk signal to server");
                            break;
                        }

                        println!("[Debug] Received chunked signal from client");
                        continue; // Optionally skip sending this message or handle it differently
                    }
                    _ => {
                        // Handle regular messages
                        let message = Message::new(
                            Some(instance_lock.0.clone()),
                            Some(get_timestamp()),
                            Some(line.clone()),
                            Some(instance_lock.1),
                        );

                        // Encrypt the message
                        let encrypted_message = encrypt_message(&key, &message)?;

                        // Send the encrypted message
                        if writer.write_all(&encrypted_message).await.is_err() {
                            eprintln!("Failed to send message to server");
                            break;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

// Set the client's name
async fn set_name() -> Result<Instance, Box<dyn StdError + Send + Sync>> {
    let name = get_user_input(Some("Enter your name: "))?;
    println!("Name set as: {}", name);
    Ok(Arc::new(Mutex::new((name, SerdeColor::Yellow))))
}

// Set the encryption key
async fn set_key() -> Result<Key, Box<dyn StdError + Send + Sync>> {
    let key = get_user_input(Some("Enter the key to connect to the server: "))?;
    Ok(Arc::new(key))
}

// Helper function to get user input from stdin
fn get_user_input(prompt: Option<&str>) -> Result<String, std::io::Error> {
    if let Some(prompt) = prompt {
        print!("> {}", prompt);
        std::io::stdout().flush()?;
    }

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

async fn print_colored_text(
    text: &str,
    color: Color,
    color_bool: ColorBool,
) -> std::io::Result<()> {
    let color_bool = color_bool.lock().await;

    match *color_bool {
        true => {
            let mut stdout = std::io::stdout();

            // Set the foreground color
            stdout.execute(SetForegroundColor(color))?;

            // Print the text
            execute!(stdout, Print(text))?;

            // \n for better readability
            execute!(stdout, Print("\n"))?;

            // Reset the terminal color to default
            stdout.execute(ResetColor)?;

            // Clear incomplete commands or previous outputs
            execute!(stdout, Clear(ClearType::FromCursorDown))?;
        }
        false => {
            let mut stdout = std::io::stdout();

            // Print the text
            execute!(stdout, Print(text))?;

            // \n for better readability
            execute!(stdout, Print("\n"))?;

            // Clear incomplete commands or previous outputs
            execute!(stdout, Clear(ClearType::FromCursorDown))?;
        }
    }

    Ok(())
}

// Function to repeatedly attempt connecting to the server
async fn wait_for_server(
    ip: &str,
    port: u16,
) -> Result<TcpStream, Box<dyn StdError + Send + Sync>> {
    let address = format!("{}:{}", ip, port);

    // Start time before the connection attempt
    let start_time = Instant::now();

    loop {
        println!("Attempting to connect to {}...", address);

        // Try to connect to the server
        match TcpStream::connect(&address).await {
            Ok(socket) => {
                println!("Successfully connected to the server!");
                return Ok(socket);
            }
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                eprintln!("Server not available");
            }
            Err(e) => {
                eprintln!("Failed to connect: {:?}", e);
            }
        }

        // Calculate the elapsed time for the connection attempt
        let elapsed = start_time.elapsed();
        let elapsed_secs = elapsed.as_secs();

        // Print the status and retry after a delay
        println!(
            "Retrying in {} second(s)... (Time elapsed: {} seconds)",
            RETRY_DELAY, elapsed_secs
        );

        if elapsed_secs >= CONNECTION_TIMEOUT {
            eprintln!("Connection timed out after {} seconds", CONNECTION_TIMEOUT);
            return Err("Connection timed out".into());
        }

        // Wait for a bit before retrying
        sleep(Duration::from_secs(RETRY_DELAY)).await;
    }
}
