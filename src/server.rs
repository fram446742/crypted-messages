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

type SharedState = Arc<Mutex<HashMap<usize, Client>>>;
pub type AssignedColors = Arc<Mutex<HashSet<SerdeColor>>>;
type Key = Arc<String>;
type SudoKey = Arc<String>;
type History = Arc<Mutex<VecDeque<Message>>>;

const SUDO_MESSAGE: &str = "
You have been granted sudo privileges.
Use /change-color to change your color.
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

    // Generate a unique client ID
    let id = get_client_id(&state).await;

    // Perform the handshake with the client to get the initial name
    let initial_name = perform_handshake(&key, &mut reader).await?;

    // Collect all names under lock, but release lock afterward for name generation
    let existing_names: Vec<String> = {
        let state_guard = state.lock().await;
        state_guard
            .values()
            .map(|client| client.name.clone())
            .collect()
    };

    // Generate a unique name by appending a counter if necessary
    let mut name = initial_name.clone();
    let mut counter = 1;
    while existing_names.contains(&name) {
        name = format!("{}-{}", initial_name, counter);
        counter += 1;
    }

    // Now the name is guaranteed to be unique, continue with client registration

    let color = assign_random_color(&state, &assigned_colors, &id).await?;

    // Register the client with an empty message history
    let (tx, rx) = broadcast::channel(10);
    let client = Client::new(name.clone(), tx, color);

    state.lock().await.insert(id, client.clone());
    println!("{} connected (ID: {})", name, id);

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
        &id,
        &mut reader,
        &writer_clone,
        sudo_key,
        color,
        history,
    )
    .await;

    // Clean up the client on disconnect
    cleanup_client(state, &name, &id).await;

    // Wait for the message task to finish
    tx_task.await?;

    result
}

// Get the client ID
async fn get_client_id(state: &SharedState) -> usize {
    let state_guard = state.lock().await;
    state_guard.len()
}

// Assign a unique random color to each client
async fn assign_random_color(
    state: &SharedState,
    assigned_colors: &AssignedColors, // External variable for assigned colors
    id: &usize,
) -> Result<SerdeColor, Box<dyn std::error::Error + Send + Sync>> {
    let mut state_lock = state.lock().await;
    let mut assigned_colors_lock = assigned_colors.lock().await;

    let mut chosen_color = random_color();

    // Keep generating until a new unassigned color is found
    while assigned_colors_lock.contains(&chosen_color) {
        chosen_color = random_color();
    }

    // Assign the color to the client in the state
    if let Some(client) = state_lock.get_mut(id) {
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

    match timeout(Duration::from_secs(60), reader.read(&mut buffer)).await {
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
    id: &usize,
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
                handle_sudo_command(
                    message,
                    name,
                    id,
                    sudo_key.clone(),
                    state,
                    key,
                    writer,
                    color,
                )
                .await?;
                continue;
            }
        }

        let client_sudo = {
            let state_guard = state.lock().await;
            state_guard
                .get(id)
                .map(|client| client.sudo)
                .unwrap_or(false)
        };

        // Handle sudo or non-sudo commands
        if let Some(message) = decrypted_msg.message.clone() {
            if client_sudo {
                handle_sudo_commands(
                    &message,
                    name,
                    id,
                    state,
                    history.clone(),
                    key,
                    writer,
                    color,
                )
                .await?;
            } else {
                handle_non_sudo_commands(&message, name, id, state, key, writer, color).await?;
            }
        }

        // Broadcast non-command messages
        if let Some(message) = &decrypted_msg.message {
            if !message.starts_with("/") {
                store_message_in_history(&decrypted_msg, id, state, history.clone()).await?;
                broadcast_message(&key, &state, id, decrypted_msg).await?;
            }
        }
    }

    Ok(())
}

// Handles the /sudo command
async fn handle_sudo_command(
    message: &str,
    name: &str,
    id: &usize,
    sudo_key: SudoKey,
    state: &SharedState,
    key: &Key,
    writer: &Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    color: SerdeColor,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let parts: Vec<&str> = message.split_whitespace().collect();
    if parts.len() == 2 && parts[1] == *sudo_key {
        let mut state_guard = state.lock().await;
        if let Some(client) = state_guard.get_mut(id) {
            client.sudo = true;
        }

        send_server_message(writer, None, key, SUDO_MESSAGE, color).await?;
        println!("{} granted sudo privileges", name);
    } else {
        send_server_message(
            writer,
            None,
            key,
            "Incorrect sudo password",
            SerdeColor::Red,
        )
        .await?;
        println!("{} provided incorrect sudo password", name);
    }
    Ok(())
}

// Sends a message from the server to the client
async fn send_server_message(
    writer: &Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    name: Option<&str>,
    key: &Key,
    message: &str,
    color: SerdeColor,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let msg = Message {
        name: Some(name.unwrap_or("Server").to_string()),
        timestamp: Some(get_timestamp()),
        message: Some(message.to_string()),
        color: Some(color),
    };

    let encrypted_msg = encrypt_message(&key, &msg)?;
    let mut writer_lock = writer.lock().await;
    writer_lock.write_all(&encrypted_msg).await?;
    writer_lock.flush().await?;
    Ok(())
}

// Handles commands when sudo privileges are granted
async fn handle_sudo_commands(
    message: &str,
    name: &str,
    id: &usize,
    state: &SharedState,
    history: History,
    key: &Key,
    writer: &Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    color: SerdeColor,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match ServerCommand::from_str(message) {
        ServerCommand::Close => handle_close_command(name, key, writer, color).await?,
        ServerCommand::ViewMessages => {
            let client_messages = get_client_message_history(id, state).await;
            send_server_message(writer, None, key, &client_messages, color).await?;
        }
        ServerCommand::ViewHistory => {
            let name = "Your chat history: \n";
            let global_messages = get_global_message_history(history).await;
            send_server_message(writer, Some(name), key, &global_messages, color).await?;
        }
        ServerCommand::ViewKey => {
            send_server_message(writer, None, key, &format!("AES Key: {}", key), color).await?;
        }
        ServerCommand::ChangeColor => {
            let color = change_client_color(id, state).await?;
            send_server_message(writer, None, key, "Color changed", color).await?;
        }
        _ => (),
    }
    Ok(())
}

// Handles non-sudo commands
async fn handle_non_sudo_commands(
    message: &str,
    name: &str,
    id: &usize,
    state: &SharedState,
    key: &Key,
    writer: &Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    color: SerdeColor,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match ServerCommand::from_str(message) {
        ServerCommand::ViewMessages => {
            let name = "Your chat history: \n";
            let client_messages = get_client_message_history(id, state).await;
            send_server_message(writer, Some(name), key, &client_messages, color).await?;
        }
        ServerCommand::ChangeColor => {
            let color = change_client_color(id, state).await?;
            send_server_message(writer, None, key, "COLOR_CHANGE", color).await?;
        }
        ServerCommand::Close => handle_close_command(name, key, writer, color).await?,
        _ => (),
    }
    Ok(())
}

// Handles the /close command
async fn handle_close_command(
    name: &str,
    key: &Key,
    writer: &Arc<Mutex<tokio::io::WriteHalf<TcpStream>>>,
    color: SerdeColor,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("{} issued /close command", name);

    send_server_message(writer, None, key, "CLOSE_CONNECTION", color).await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    Ok(())
}

// Fetches the client's message history
async fn get_client_message_history(id: &usize, state: &SharedState) -> String {
    let state_guard = state.lock().await;
    if let Some(client) = state_guard.get(id) {
        return match client.get_messages() {
            Ok(messages) => messages,
            Err(_) => "No message history".to_string(),
        };
    }
    format!("Couldn't get client with id {}", id)
}

// Fetches the global message history
async fn get_global_message_history(history: History) -> String {
    let history_guard = history.lock().await;
    history_guard
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
        .join("\n")
}

// Changes the client's color
async fn change_client_color(
    id: &usize,
    state: &SharedState,
) -> Result<SerdeColor, Box<dyn std::error::Error + Send + Sync>> {
    let mut state_guard = state.lock().await;
    let color = random_color();
    if let Some(client) = state_guard.get_mut(id) {
        client.color = color;
    }
    Ok(color)
}

// Stores the message in both client and global history
async fn store_message_in_history(
    message: &Message,
    id: &usize,
    state: &SharedState,
    history: History,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut state_guard = state.lock().await;
    if let Some(client) = state_guard.get_mut(id) {
        client.add_message(message.clone());
    }
    history.lock().await.push_back(message.clone());
    Ok(())
}

// Clean up the client on disconnect
async fn cleanup_client(state: SharedState, name: &str, id: &usize) {
    state.lock().await.remove(id);
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
    sender_id: &usize,
    msg: Message,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let encrypted_message = encrypt_message(&key, &msg)?;

    let state = state.lock().await;
    for (client_id, client) in state.iter() {
        if client_id != sender_id {
            let _ = client.tx.send(encrypted_message.clone());
        }
    }

    Ok(())
}
