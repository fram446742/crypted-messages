use local_ip_address::local_ip;
use rand::Rng;
use std::collections::HashSet;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio::task;
use tokio::time::{timeout, Duration};

use crate::tools::{
    decrypt_handshake, decrypt_message, encrypt_handshake, encrypt_message, generate_key,
    get_timestamp, Client, Handshake, Message, SerdeColor, ServerCommand,
};
use crate::tools::{get_ip, get_port, random_color, AdressMode};

type SharedState = Arc<Mutex<HashMap<String, Client>>>;
pub type AssignedColors = Arc<Mutex<HashSet<SerdeColor>>>;
type Key = Arc<String>;
type SudoKey = Arc<String>;
type History = Arc<Mutex<VecDeque<Message>>>;

// TODO: Fix the timestamp to show a more human-readable format
const SUDO_MESSAGE: &str = "
You have been granted sudo privileges.
Use /close to close the connection.
Use /view-messages to view your messages.
Use /view-history to view global chat history.
Use /view-key to view the AES key.
";

pub async fn main_server(
    key: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Ask for the IP address and port to bind the server to
    let ip = get_ip(
        None,
        Some("Enter the IP address (leave blank if unsure): "),
        AdressMode::Server,
    )?;
    let port = get_port(
        None,
        Some("Enter the port to bind the server to (leave blank for OS assign): "),
        AdressMode::Server,
    )?;

    let listener = setup_tcp_listener(ip, port).await?;

    // Generate the SharedState and Key
    let key: Key = Arc::new(set_aes_key(key));
    let sudo_key: SudoKey = Arc::new(set_sudo_key());
    let state: SharedState = Arc::new(Mutex::new(HashMap::new()));
    let assigned_colors: AssignedColors = Arc::new(Mutex::new(HashSet::new()));
    let history: History = Arc::new(Mutex::new(VecDeque::new()));

    // Main loop to accept incoming connections
    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                spawn_client_handler(
                    socket,
                    state.clone(),
                    key.clone(),
                    sudo_key.clone(),
                    assigned_colors.clone(),
                    history.clone(),
                );
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {:?}", e);
            }
        }
    }
}

// Helper function to generate an AES key
fn set_aes_key(key: Option<String>) -> String {
    key.unwrap_or_else(|| {
        let server_key = generate_key(32);
        println!("[SERVER] Generated server key: {}", server_key);
        server_key
    })
}

fn set_sudo_key() -> String {
    let code = rand::thread_rng().gen_range(1000..9999).to_string();
    println!("[SERVER] Sudo code generated: {}", code);
    code
}

// Setup the TCP listener
async fn setup_tcp_listener(
    ip: String,
    mut port: u16,
) -> Result<TcpListener, Box<dyn std::error::Error + Send + Sync>> {
    loop {
        println!("[SERVER] Binding to {}:{}", ip, port);
        match TcpListener::bind(format!("{}:{}", ip, port)).await {
            Ok(listener) => {
                println!(
                    "[SERVER] Running on {}:{}, public IP: {}. Waiting for connections...",
                    listener.local_addr().unwrap().ip(),
                    listener.local_addr().unwrap().port(),
                    local_ip().unwrap_or_else(|_| "Unknown ip".parse().unwrap()),
                );
                return Ok(listener);
            }
            Err(e) => {
                eprintln!("Failed to bind to port {}: {}. ", port, e);
                eprintln!("Trying port {}.", port + 10);
                port += 10;
            }
        }
    }
}

// Spawns a task to handle the client connection
fn spawn_client_handler(
    socket: TcpStream,
    state: SharedState,
    key: Key,
    sudo_key: SudoKey,
    assigned_colors: AssignedColors,
    history: History,
) {
    task::spawn(async move {
        if let Err(e) = handle_client(socket, state, key, sudo_key, assigned_colors, history).await
        {
            eprintln!("Failed to handle client: {:?}", e);
        }
    });
}

// Handle the client connection
async fn handle_client(
    socket: TcpStream,
    state: SharedState,
    key: Key,
    sudo_key: SudoKey,
    assigned_colors: AssignedColors,
    history: History,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, writer) = tokio::io::split(socket);
    let mut reader = BufReader::new(reader);
    let writer = Arc::new(Mutex::new(writer));

    // Perform the handshake with a 10-second timeout
    let name = perform_handshake(&key, &mut reader).await?;

    let color = assign_random_color(&state, &assigned_colors, &name).await?;

    // Register the client with an empty message history
    let (tx, rx) = broadcast::channel(10);
    let client = Client {
        name: name.clone(),
        tx,
        color, // Default color, can be customized
        messages: VecDeque::new(),
        sudo: false,
    };
    state.lock().await.insert(name.clone(), client.clone());
    println!("{} connected", name);

    // Send handshake response and welcome message
    send_handshake_response(&key, &name, &writer, color).await?;
    send_welcome_message(&key, &name, &writer, color).await?;

    // Spawn task to handle outgoing messages
    let writer_clone = Arc::clone(&writer);
    let tx_task = spawn_message_sender(writer_clone.clone(), rx, &name);

    // Main loop to handle incoming messages
    let result = handle_incoming_messages(
        &key,
        &state,
        &name,
        &mut reader,
        &writer_clone,
        sudo_key,
        color,
        history,
    )
    .await;

    // Clean up the client on disconnect
    cleanup_client(state, &name).await;

    // Wait for the message task to finish
    tx_task.await?;

    result
}

// Assign a unique random color to each client
async fn assign_random_color(
    state: &SharedState,
    assigned_colors: &AssignedColors, // External variable for assigned colors
    name: &str,
) -> Result<SerdeColor, Box<dyn std::error::Error + Send + Sync>> {
    let mut state_lock = state.lock().await;
    let mut assigned_colors_lock = assigned_colors.lock().await;

    let mut chosen_color = random_color();

    // Keep generating until a new unassigned color is found
    while assigned_colors_lock.contains(&chosen_color) {
        chosen_color = random_color();
    }

    // Assign the color to the client in the state
    if let Some(client) = state_lock.get_mut(name) {
        client.color = chosen_color.clone();
        assigned_colors_lock.insert(chosen_color.clone());
    }

    Ok(chosen_color)
}

// Perform the handshake process
async fn perform_handshake(
    key: &Key,
    reader: &mut BufReader<tokio::io::ReadHalf<TcpStream>>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut buffer = [0u8; 1024];

    match timeout(Duration::from_secs(10), reader.read(&mut buffer)).await {
        Ok(Ok(n)) if n > 0 => {
            let handshake = decrypt_handshake(&key, &buffer[..n])?;
            Ok(handshake.name)
        }
        _ => {
            eprintln!("Handshake failed or timed out");
            Err("Handshake failed or timed out".into())
        }
    }
}

// Spawn a task to send messages to the client
fn spawn_message_sender(
    writer: Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    mut rx: broadcast::Receiver<Vec<u8>>,
    name: &str,
) -> tokio::task::JoinHandle<()> {
    let name_clone = name.to_string();
    tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let mut writer_lock = writer.lock().await;
            if writer_lock.write_all(&msg).await.is_err() {
                eprintln!("Failed to send message to {}: {:?}", name_clone, msg);
                break;
            }
        }
    })
}

// Handle incoming messages from the client
async fn handle_incoming_messages(
    key: &Key,
    state: &SharedState,
    name: &str,
    reader: &mut BufReader<tokio::io::ReadHalf<TcpStream>>,
    writer: &Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    sudo_key: SudoKey,
    color: SerdeColor,
    history: History,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buffer = [0u8; 1024];

    while let Ok(n) = reader.read(&mut buffer).await {
        if n == 0 {
            break; // Client disconnected
        }
        let decrypted_msg = decrypt_message(&key, &buffer[..n])?;
        println!("{}: {:?}", name, decrypted_msg);

        // Handle the /sudo command
        if let Some(message) = &decrypted_msg.message {
            if message.starts_with("/sudo") {
                let parts: Vec<&str> = message.split_whitespace().collect();
                if parts.len() == 2 && parts[1] == *sudo_key {
                    // Grant sudo privileges
                    if let Some(client) = state.lock().await.get_mut(name) {
                        client.sudo = true;
                    }

                    // Send a confirmation message to the client
                    let sudo_msg = Message {
                        name: Some("Server".to_string()),
                        timestamp: Some(get_timestamp()),
                        message: Some(SUDO_MESSAGE.to_string()),
                        color: Some(color),
                    };

                    let encrypted_msg = encrypt_message(&key, &sudo_msg)?;
                    let mut writer_lock = writer.lock().await;
                    writer_lock.write_all(&encrypted_msg).await?;
                    writer_lock.flush().await?;

                    println!("{} granted sudo privileges", name);
                } else {
                    // Send error message for incorrect sudo password
                    let error_msg = Message {
                        name: Some("Server".to_string()),
                        timestamp: Some(get_timestamp()),
                        message: Some("Incorrect sudo password".to_string()),
                        color: Some(SerdeColor::Red),
                    };

                    let encrypted_msg = encrypt_message(&key, &error_msg)?;
                    let mut writer_lock = writer.lock().await;
                    writer_lock.write_all(&encrypted_msg).await?;
                    writer_lock.flush().await?;

                    println!("{} provided incorrect sudo password", name);
                }
                continue; // Do not broadcast, move to the next message
            }
        }

        // Handle other commands if sudo is granted
        if let Some(client) = state.lock().await.get(name) {
            if client.sudo {
                let message_content = decrypted_msg.message.clone().unwrap_or_default();
                match ServerCommand::from_str(&message_content) {
                    ServerCommand::Close => {
                        println!("{} issued /close command", name);

                        let close_signal = Message {
                            name: Some("Server".to_string()),
                            timestamp: Some(get_timestamp()),
                            message: Some("CLOSE_SIGNAL".to_string()),
                            color: Some(color),
                        };

                        let encrypted_signal = encrypt_message(&key, &close_signal)?;
                        let mut writer_lock = writer.lock().await;
                        writer_lock.write_all(&encrypted_signal).await?;
                        writer_lock.flush().await?;

                        tokio::time::sleep(Duration::from_secs(1)).await;
                        break; // Close the connection
                    }
                    ServerCommand::ViewMessages => {
                        // Display message history to the client
                        let client_messages = client
                            .messages
                            .iter()
                            .map(|msg| {
                                format!(
                                    "{}: {}",
                                    msg.timestamp.clone().unwrap_or("Unknown time".to_string()),
                                    msg.message.as_deref().unwrap_or("")
                                )
                            })
                            .collect::<Vec<String>>()
                            .join("\n");

                        let view_msg = Message {
                            name: Some("Server".to_string()),
                            timestamp: Some(get_timestamp()),
                            message: Some(client_messages),
                            color: Some(color),
                        };

                        let encrypted_msg = encrypt_message(&key, &view_msg)?;
                        let mut writer_lock = writer.lock().await;
                        writer_lock.write_all(&encrypted_msg).await?;
                        writer_lock.flush().await?;
                    }
                    ServerCommand::ViewHistory => {
                        // Display global message history to the client
                        let global_messages = history
                            .lock()
                            .await
                            .iter()
                            .map(|msg| {
                                format!(
                                    "{}: {}: {}",
                                    msg.name.clone().unwrap_or("Unknown".to_string()),
                                    msg.timestamp.clone().unwrap_or("Unknown time".to_string()),
                                    msg.message.as_deref().unwrap_or("")
                                )
                            })
                            .collect::<Vec<String>>()
                            .join("\n");

                        let view_msg = Message {
                            name: None,
                            timestamp: Some(get_timestamp()),
                            message: Some(global_messages),
                            color: Some(color),
                        };

                        let encrypted_msg = encrypt_message(&key, &view_msg)?;
                        let mut writer_lock = writer.lock().await;
                        writer_lock.write_all(&encrypted_msg).await?;
                        writer_lock.flush().await?;
                    }
                    ServerCommand::ViewKey => {
                        // Display the AES key to the client
                        let key_msg = Message {
                            name: Some("Server".to_string()),
                            timestamp: Some(get_timestamp()),
                            message: Some(format!("AES Key: {}", key)),
                            color: Some(color),
                        };

                        let encrypted_msg = encrypt_message(&key, &key_msg)?;
                        let mut writer_lock = writer.lock().await;
                        writer_lock.write_all(&encrypted_msg).await?;
                        writer_lock.flush().await?;
                    }
                    ServerCommand::ChangeColor => {
                        // Change the client's color
                        let new_color = random_color();
                        if let Some(client) = state.lock().await.get_mut(name) {
                            client.color = new_color.clone();
                        }

                        // Send a confirmation message to the client
                        let color_msg = Message {
                            name: Some("Server".to_string()),
                            timestamp: Some(get_timestamp()),
                            message: Some(format!("Color changed to {:?}", new_color)),
                            color: Some(new_color),
                        };

                        let encrypted_msg = encrypt_message(&key, &color_msg)?;
                        let mut writer_lock = writer.lock().await;
                        writer_lock.write_all(&encrypted_msg).await?;
                        writer_lock.flush().await?;
                    }
                    ServerCommand::Invalid => {
                        // Invalid command, just continue without broadcasting
                    }
                }
                // continue; // Do not broadcast any command
            } else {
                let message_content = decrypted_msg.message.clone().unwrap_or_default();
                match ServerCommand::from_str(&message_content) {
                    ServerCommand::ViewMessages => {
                        // Display message history to the client
                        let client_messages = client
                            .messages
                            .iter()
                            .map(|msg| {
                                format!(
                                    "{}: {}",
                                    msg.timestamp.clone().unwrap_or("Unknown time".to_string()),
                                    msg.message.as_deref().unwrap_or("")
                                )
                            })
                            .collect::<Vec<String>>()
                            .join("\n");

                        let view_msg = Message {
                            name: Some("Server".to_string()),
                            timestamp: Some(get_timestamp()),
                            message: Some(client_messages),
                            color: Some(color),
                        };

                        let encrypted_msg = encrypt_message(&key, &view_msg)?;
                        let mut writer_lock = writer.lock().await;
                        writer_lock.write_all(&encrypted_msg).await?;
                        writer_lock.flush().await?;
                    }
                    ServerCommand::ChangeColor => {
                        // Change the client's color
                        let new_color = random_color();
                        if let Some(client) = state.lock().await.get_mut(name) {
                            client.color = new_color.clone();
                        }

                        // Send a confirmation message to the client
                        let color_msg = Message {
                            name: Some("Server".to_string()),
                            timestamp: Some(get_timestamp()),
                            message: Some(format!("Color changed to {:?}", new_color)),
                            color: Some(new_color),
                        };

                        let encrypted_msg = encrypt_message(&key, &color_msg)?;
                        let mut writer_lock = writer.lock().await;
                        writer_lock.write_all(&encrypted_msg).await?;
                        writer_lock.flush().await?;
                    }
                    ServerCommand::Close => {
                        println!("{} issued /close command", name);

                        let close_signal = Message {
                            name: Some("Server".to_string()),
                            timestamp: Some(get_timestamp()),
                            message: Some("CLOSE_SIGNAL".to_string()),
                            color: Some(color),
                        };

                        let encrypted_signal = encrypt_message(&key, &close_signal)?;
                        let mut writer_lock = writer.lock().await;
                        writer_lock.write_all(&encrypted_signal).await?;
                        writer_lock.flush().await?;

                        tokio::time::sleep(Duration::from_secs(1)).await;
                        break; // Close the connection
                    }
                    _ => (),
                }
            }
        }

        // If the message is not a command, broadcast it
        if let Some(message) = &decrypted_msg.message {
            if !message.starts_with("/") {
                // Store the message in the client's message history
                if let Some(client) = state.lock().await.get_mut(name) {
                    client.messages.push_back(decrypted_msg.clone());
                }

                // Store the message in the global history
                history.lock().await.push_back(decrypted_msg.clone());

                // Broadcast the message to other clients
                broadcast_message(&key, &state, &name, decrypted_msg).await?;
            }
        }
    }

    Ok(())
}

// Clean up the client on disconnect
async fn cleanup_client(state: SharedState, name: &str) {
    state.lock().await.remove(name);
    println!("{} disconnected", name);
}

// Send the handshake response to the client with a color
async fn send_handshake_response(
    key: &Key,
    name: &str,
    writer: &Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    color: SerdeColor,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let handshake = Handshake::new(name.to_string(), 1024, Some(color));
    let encrypted_handshake = encrypt_handshake(&key, &handshake)?;

    let mut writer_lock = writer.lock().await;
    writer_lock.write_all(&encrypted_handshake).await?;
    writer_lock.flush().await?;
    Ok(())
}

// Send a welcome message to the client
async fn send_welcome_message(
    key: &Key,
    name: &str,
    writer: &Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    color: SerdeColor,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let welcome_msg = Message {
        name: Some("Server".to_string()),
        timestamp: Some(get_timestamp()),
        message: Some(format!(
            "Welcome {} to the chat! Use /help for available commands",
            name
        )),
        color: Some(color),
    };

    let encrypted_msg = encrypt_message(&key, &welcome_msg)?;

    let mut writer_lock = writer.lock().await;
    writer_lock.write_all(&encrypted_msg).await?;
    writer_lock.flush().await?;
    Ok(())
}

// Broadcast the message to all clients except the sender
async fn broadcast_message(
    key: &Key,
    state: &SharedState,
    sender_name: &str,
    msg: Message,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let encrypted_message = encrypt_message(&key, &msg)?;

    let state = state.lock().await;
    for (client_name, client) in state.iter() {
        if client_name != sender_name {
            let _ = client.tx.send(encrypted_message.clone());
        }
    }

    Ok(())
}
