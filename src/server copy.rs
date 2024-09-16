use local_ip_address::local_ip;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio::task;
use tokio::time::{timeout, Duration};

use crate::tools::{
    decrypt_handshake, decrypt_message, encrypt_handshake, encrypt_message, generate_key,
    get_timestamp, Handshake, Message, SerdeColor,
};

type SharedState = Arc<Mutex<HashMap<String, broadcast::Sender<Vec<u8>>>>>; 
type Key = Arc<String>;

pub async fn main_server(
    key: Option<String>,
    port: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = setup_tcp_listener(port).await?;

    // Generate the SharedState and Key
    let key: Key = Arc::new(set_aes_key(key));
    let state: SharedState = Arc::new(Mutex::new(HashMap::new()));

    // Main loop to accept incoming connections
    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                spawn_client_handler(socket, state.clone(), key.clone());
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

// Setup the TCP listener
async fn setup_tcp_listener(
    port: Option<String>,
) -> Result<TcpListener, Box<dyn std::error::Error + Send + Sync>> {
    let port = port
        .unwrap_or_else(|| "5555".to_string())
        .parse()
        .unwrap_or(5555);
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

    println!(
        "[SERVER] Running on {}:{}",
        local_ip().unwrap_or_else(|_| "0.0.0.0".parse().unwrap()),
        port
    );
    Ok(listener)
}

// Spawns a task to handle the client connection
fn spawn_client_handler(socket: TcpStream, state: SharedState, key: Key) {
    task::spawn(async move {
        if let Err(e) = handle_client(socket, state, key).await {
            eprintln!("Failed to handle client: {:?}", e);
        }
    });
}

// Handle the client connection
async fn handle_client(
    socket: TcpStream,
    state: SharedState,
    key: Key,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, writer) = tokio::io::split(socket);
    let mut reader = BufReader::new(reader);
    let writer = Arc::new(Mutex::new(writer));

    // Perform the handshake with a 10-second timeout
    let name = perform_handshake(&key, &mut reader).await?;

    // Register the client
    let (tx, rx) = broadcast::channel(10);
    state.lock().await.insert(name.clone(), tx);
    println!("{} connected", name);

    // Send handshake response and welcome message
    send_handshake_response(&key, &name, &writer).await?;
    send_welcome_message(&key, &name, &writer).await?;

    // Spawn task to handle outgoing messages
    let writer_clone = Arc::clone(&writer);
    let tx_task = spawn_message_sender(writer_clone, rx, &name);

    // Main loop to handle incoming messages
    let result = handle_incoming_messages(&key, &state, &name, &mut reader).await;

    // Clean up the client on disconnect
    cleanup_client(state, &name).await;

    // Wait for the message task to finish
    tx_task.await?;

    result
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
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buffer = [0u8; 1024];

    while let Ok(n) = reader.read(&mut buffer).await {
        if n == 0 {
            break; // Client disconnected
        }
        let decrypted_msg = decrypt_message(&key, &buffer[..n])?;
        println!("{}: {:?}", name, decrypted_msg);
        broadcast_message(&key, &state, &name, decrypted_msg).await?;
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
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let handshake = Handshake::new(name.to_string(), 1024, Some(SerdeColor::Green));
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
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let welcome_msg = Message {
        name: Some("Server".to_string()),
        timestamp: Some(get_timestamp()),
        message: Some(format!("Welcome {} to the chat!", name)),
        color: Some(SerdeColor::Green),
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
    for (client_name, tx) in state.iter() {
        if client_name != sender_name {
            let _ = tx.send(encrypted_message.clone());
        }
    }

    Ok(())
}
