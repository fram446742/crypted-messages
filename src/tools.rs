use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::Key;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{anyhow, Context, Ok, Result};
use crossterm::style::Color;
// Import anyhow for error handling
use chrono::prelude::*;
use rand::{Rng, RngCore};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::net::IpAddr;
use tokio::sync::broadcast;

pub enum AdressMode {
    Server,
    Client,
}
// Estructura del mensaje
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Message {
    pub name: Option<String>,
    pub timestamp: Option<String>,
    pub message: Option<String>,
    pub color: Option<SerdeColor>,
}

impl Message {
    pub fn new(
        name: Option<String>,
        timestamp: Option<String>,
        message: Option<String>,
        color: Option<SerdeColor>,
    ) -> Self {
        Message {
            name,
            timestamp,
            message,
            color,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct MessageData {
    pub message: Message,
    pub sender_id: usize,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Handshake {
    pub name: String,
    pub buffer_size: usize,
    pub color: Option<SerdeColor>,
}

impl Handshake {
    pub fn new(name: String, buffer_size: usize, color: Option<SerdeColor>) -> Self {
        Handshake {
            name,
            buffer_size,
            color,
        }
    }
}

// Estructura del cliente
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Client {
    pub name: String,
    pub tx: broadcast::Sender<Vec<u8>>,
    pub color: SerdeColor,
    pub messages: VecDeque<Message>, // Efficient data structure for storing messages
    pub sudo: bool,
}

pub enum ServerCommand {
    Close,
    ViewMessages,
    ViewHistory,
    ViewKey,
    Invalid,
}

impl ServerCommand {
    pub fn from_str(command: &str) -> Self {
        match command {
            "/close" => ServerCommand::Close,
            "/view-messages" => ServerCommand::ViewMessages,
            "/view-key" => ServerCommand::ViewKey,
            "/view-history" => ServerCommand::ViewHistory,
            _ => ServerCommand::Invalid,
        }
    }
}

pub enum ClientCommand {
    Quit,
    ToogleColor,
    Help,
    Invalid,
}

impl ClientCommand {
    pub fn from_str(command: &str) -> Self {
        match command {
            "/quit" => ClientCommand::Quit,
            "/toggle-color" => ClientCommand::ToogleColor,
            "/help" => ClientCommand::Help,
            _ => ClientCommand::Invalid,
        }
    }
}

// Colores personalizados
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SerdeColor {
    Red,
    DarkRed,
    Green,
    DarkGreen,
    Blue,
    DarkBlue,
    White,
    Black,
    Yellow,
    DarkYellow,
    Cyan,
    DarkCyan,
    Magenta,
    DarkMagenta,
    Grey,
    DarkGrey,
    Custom(u8, u8, u8), // RGB colors
    AnsiValue(u8),      // For 8-bit ANSI values
}

// Convert crossterm::style::Color to the appropriate SerdeColor when needed
impl From<crossterm::style::Color> for SerdeColor {
    fn from(color: crossterm::style::Color) -> Self {
        match color {
            crossterm::style::Color::Red => SerdeColor::Red,
            crossterm::style::Color::DarkRed => SerdeColor::DarkRed,
            crossterm::style::Color::Green => SerdeColor::Green,
            crossterm::style::Color::DarkGreen => SerdeColor::DarkGreen,
            crossterm::style::Color::Blue => SerdeColor::Blue,
            crossterm::style::Color::DarkBlue => SerdeColor::DarkBlue,
            crossterm::style::Color::White => SerdeColor::White,
            crossterm::style::Color::Black => SerdeColor::Black,
            crossterm::style::Color::Yellow => SerdeColor::Yellow,
            crossterm::style::Color::DarkYellow => SerdeColor::DarkYellow,
            crossterm::style::Color::Cyan => SerdeColor::Cyan,
            crossterm::style::Color::DarkCyan => SerdeColor::DarkCyan,
            crossterm::style::Color::Magenta => SerdeColor::Magenta,
            crossterm::style::Color::DarkMagenta => SerdeColor::DarkMagenta,
            crossterm::style::Color::Grey => SerdeColor::Grey,
            crossterm::style::Color::DarkGrey => SerdeColor::DarkGrey,
            crossterm::style::Color::Rgb { r, g, b } => SerdeColor::Custom(r, g, b),
            crossterm::style::Color::AnsiValue(val) => SerdeColor::AnsiValue(val),
            // Fallback for unhandled colors
            _ => SerdeColor::White,
        }
    }
}

// Convert SerdeColor to the appropriate crossterm::style::Color when needed
impl From<SerdeColor> for crossterm::style::Color {
    fn from(color: SerdeColor) -> Self {
        match color {
            SerdeColor::Red => crossterm::style::Color::Red,
            SerdeColor::DarkRed => crossterm::style::Color::DarkRed,
            SerdeColor::Green => crossterm::style::Color::Green,
            SerdeColor::DarkGreen => crossterm::style::Color::DarkGreen,
            SerdeColor::Blue => crossterm::style::Color::Blue,
            SerdeColor::DarkBlue => crossterm::style::Color::DarkBlue,
            SerdeColor::White => crossterm::style::Color::White,
            SerdeColor::Black => crossterm::style::Color::Black,
            SerdeColor::Yellow => crossterm::style::Color::Yellow,
            SerdeColor::DarkYellow => crossterm::style::Color::DarkYellow,
            SerdeColor::Cyan => crossterm::style::Color::Cyan,
            SerdeColor::DarkCyan => crossterm::style::Color::DarkCyan,
            SerdeColor::Magenta => crossterm::style::Color::Magenta,
            SerdeColor::DarkMagenta => crossterm::style::Color::DarkMagenta,
            SerdeColor::Grey => crossterm::style::Color::Grey,
            SerdeColor::DarkGrey => crossterm::style::Color::DarkGrey,
            SerdeColor::Custom(r, g, b) => crossterm::style::Color::Rgb { r, g, b },
            SerdeColor::AnsiValue(val) => crossterm::style::Color::AnsiValue(val),
        }
    }
}

// loogger
// #[derive(Clone)]
// pub enum Mode {
//     Foreground,
//     Background,
// }

// #[derive(Clone)]
// pub struct Logger {
//     pub mode: Mode,
//     pub log_buffer: Arc<Mutex<Vec<String>>>,
// }

// impl Logger {
//     pub fn new(mode: Mode) -> Logger {
//         Logger {
//             mode,
//             log_buffer: Arc::new(Mutex::new(Vec::new())),
//         }
//     }

//     // Logs a message based on the mode
//     pub fn log(&self, message: &str) {
//         match self.mode {
//             Mode::Foreground => {
//                 println!("{}", message); // Display the message immediately
//             }
//             Mode::Background => {
//                 let mut log_buffer = self.log_buffer.lock().unwrap();
//                 log_buffer.push(message.to_string()); // Save the message in buffer
//             }
//         }
//     }

//     // Switch logger mode
//     #[allow(dead_code)]
//     pub fn set_mode(&mut self, mode: Mode) {
//         self.mode = mode;
//     }

//     // Display saved logs
//     #[allow(dead_code)]
//     pub fn show_saved_logs(&self) {
//         let log_buffer = self.log_buffer.lock().unwrap();
//         for log in log_buffer.iter() {
//             println!("{}", log);
//         }
//     }

//     // Save logs to a file
//     #[allow(dead_code)]
//     pub fn save_to_file(&self, file_path: &str) {
//         let log_buffer = self.log_buffer.lock().unwrap();
//         let mut file = OpenOptions::new()
//             .create(true)
//             .append(true)
//             .open(file_path)
//             .unwrap();

//         for log in log_buffer.iter() {
//             writeln!(file, "{}", log).unwrap();
//         }
//     }
// }

// Conversión de HSL a RGB usando crossterm::style::Color
fn hsl_to_termcolor(h: f64, s: f64, l: f64) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    let r = ((r + m) * 255.0).round() as u8;
    let g = ((g + m) * 255.0).round() as u8;
    let b = ((b + m) * 255.0).round() as u8;

    Color::Rgb { r, g, b }
}

// Function to generate a random color
pub fn random_color() -> SerdeColor {
    let mut rng = rand::thread_rng();
    SerdeColor::from(hsl_to_termcolor(
        rng.gen_range(0.0..360.0), // Full range for hue
        rng.gen_range(0.25..1.0),  // Wider range for saturation
        rng.gen_range(0.25..0.75), // Wider range for lightness
    ))
}

pub fn get_ip(ip: Option<&str>, message: Option<&str>, mode: AdressMode) -> Result<String> {
    if let Some(ip) = ip {
        println!("[Tools-Debug] Destination IP: {}", ip);
        Ok(ip.to_string())
    } else {
        loop {
            if let Some(ref message) = message {
                println!("> {}", message);
            } else {
                match mode {
                    AdressMode::Server => {
                        println!("Enter Destination IP (leave blank for 0.0.0.0): ")
                    }
                    AdressMode::Client => {
                        println!("Enter Destination IP (leave blank for 127.0.0.1): ")
                    }
                }
            }
            io::stdout().flush().context("Failed to flush stdout")?;
            let mut host = String::new();
            io::stdin()
                .read_line(&mut host)
                .context("Failed to read line from stdin")?;
            let host = host.trim();
            let default_ip = match mode {
                AdressMode::Server => "0.0.0.0",
                AdressMode::Client => "127.0.0.1",
            };
            let host = if host.is_empty() { default_ip } else { host };

            if host.parse::<IpAddr>().is_ok() {
                return Ok(host.to_string());
            } else {
                println!("Invalid IP address.");
            }
        }
    }
}

pub fn get_port(port: Option<String>, message: Option<&str>, mode: AdressMode) -> Result<u16> {
    if let Some(port_str) = port {
        port_str.parse::<u16>().context("Invalid port number")
    } else {
        loop {
            if let Some(ref message) = message {
                println!("> {}", message);
            } else {
                match mode {
                    AdressMode::Server => println!("Enter destination port (leave blank for 0): "),
                    AdressMode::Client => {
                        println!("Enter destination port (leave blank for 5555): ")
                    }
                }
            }
            io::stdout().flush().context("Failed to flush stdout")?;
            let mut port_str = String::new();
            io::stdin()
                .read_line(&mut port_str)
                .context("Failed to read line from stdin")?;
            let port_str = port_str.trim();

            let default_port = match mode {
                AdressMode::Server => 0,
                AdressMode::Client => 5555,
            };

            if port_str.is_empty() {
                return Ok(default_port);
            }

            match port_str.parse::<u16>() {
                Result::Ok(port_num) => return Ok(port_num),
                Err(_) => println!("Invalid port number."),
            }
        }
    }
}

pub fn get_timestamp() -> String {
    let local: DateTime<Local> = Local::now();
    let year = local.year();
    let month = local.month();
    let day = local.day();
    let hour = local.hour();
    let minute = local.minute();
    let second = local.second();

    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hour, minute, second)
}

pub fn get_user_input(prompt: Option<&str>) -> String {
    if let Some(prompt) = prompt {
        print!("> {}", prompt);
    }
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

pub fn generate_key(length: usize) -> String {
    let mut key = vec![0u8; length];
    rand::thread_rng().fill_bytes(&mut key);
    hex::encode(key)
}

// Convert a hexadecimal string to a byte array
fn hex_to_bytes(hex_str: &str) -> Result<Vec<u8>> {
    hex::decode(hex_str).map_err(|e| anyhow!("Hex decode error: {:?}", e))
}

// Convert a byte array to a hexadecimal string
#[allow(dead_code)]
fn bytes_to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

// Encrypt a serializable struct
pub fn encrypt<T: Serialize>(key_str: &str, data: &T) -> Result<Vec<u8>> {
    let key = hex_to_bytes(key_str)?;

    // Serialize the struct to a JSON string
    let mut serialized_data = serde_json::to_vec(data)?;

    // Trim the data if it exceeds 1024 bytes
    if serialized_data.len() > 1024 {
        serialized_data.truncate(1024);
    }

    // Generate a random 12-byte nonce
    let nonce = {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        nonce
    };

    // Initialize the cipher
    let cipher = Aes256Gcm::new(Key::<aes_gcm::aes::Aes256>::from_slice(&key));
    let nonce = Nonce::from_slice(&nonce);

    // Encrypt the serialized data
    let ciphertext = cipher
        .encrypt(nonce, serialized_data.as_ref())
        .map_err(|e| anyhow!("Encryption error: {:?}", e))?;

    // Append the nonce to the ciphertext (needed for decryption)
    let mut result = nonce.to_vec();
    result.extend(ciphertext);

    Ok(result)
}

// Decrypt to a struct
pub fn decrypt<T: for<'de> Deserialize<'de>>(key_str: &str, ciphertext: &[u8]) -> Result<T> {
    let key = hex_to_bytes(key_str)?;

    // Split the nonce and the ciphertext
    let (nonce_bytes, ciphertext) = ciphertext.split_at(12);

    // Initialize the cipher
    let cipher = Aes256Gcm::new(Key::<aes_gcm::aes::Aes256>::from_slice(&key));
    let nonce = Nonce::from_slice(nonce_bytes);

    // Decrypt the ciphertext
    let decrypted_data = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("Encryption error: {:?}", e))?;

    // Deserialize the decrypted data
    let data: T = serde_json::from_slice(&decrypted_data)?;

    Ok(data)
}

// Encrypt a Message
pub fn encrypt_message(key_str: &str, message: &Message) -> Result<Vec<u8>> {
    encrypt(key_str, message)
}

// Decrypt a Message
pub fn decrypt_message(key_str: &str, ciphertext: &[u8]) -> Result<Message> {
    decrypt(key_str, ciphertext)
}

// Encrypt a Handshake
pub fn encrypt_handshake(key_str: &str, handshake: &Handshake) -> Result<Vec<u8>> {
    encrypt(key_str, handshake)
}

// Decrypt a Handshake
pub fn decrypt_handshake(key_str: &str, ciphertext: &[u8]) -> Result<Handshake> {
    decrypt(key_str, ciphertext)
}

#[cfg(test)]

mod tests {
    use super::*;
    use anyhow::Result;
    use regex::Regex;
    use serde::{Deserialize, Serialize};
    // Define a struct for testing purposes
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        message: String,
        number: u32,
    }

    #[test]
    fn test_aes_encryption_decryption() -> Result<()> {
        // Define a shared key (32 bytes for AES-256)
        let key = generate_key(32);

        // Create a Message
        let message = Message {
            name: Some("Alice".to_string()),
            timestamp: Some("2024-09-12T12:34:56Z".to_string()),
            message: Some("Hello, Bob!".to_string()),
            color: Some(SerdeColor::Red),
        };

        // Encrypt and Decrypt a Message
        let encrypted_message = encrypt_message(&key, &message)?;
        println!("Encrypted Message: {:?}", encrypted_message);

        let decrypted_message = decrypt_message(&key, &encrypted_message)?;
        println!("Decrypted Message: {:?}", decrypted_message);

        // Create a Handshake
        let handshake = Handshake {
            name: "Alice".to_string(),
            buffer_size: 1024,
            color: Some(SerdeColor::Blue),
        };

        // Encrypt and Decrypt a Handshake
        let encrypted_handshake = encrypt_handshake(&key, &handshake)?;
        println!("Encrypted Handshake: {:?}", encrypted_handshake);

        let decrypted_handshake = decrypt_handshake(&key, &encrypted_handshake)?;
        println!("Decrypted Handshake: {:?}", decrypted_handshake);

        // Verify the decrypted data matches the original data
        assert_eq!(message, decrypted_message);
        assert_eq!(handshake, decrypted_handshake);

        Ok(())
    }

    #[test]
    fn test_get_timestamp_format() {
        // Get the current timestamp string
        let timestamp = get_timestamp();

        // Define the regex pattern for the timestamp (YYYY-MM-DD HH:MM::SS)
        let re = Regex::new(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$").unwrap();
        // let re = Regex::new(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$").unwrap();

        // Assert that the timestamp matches the regex pattern
        assert!(re.is_match(&timestamp), "Timestamp format is incorrect");

        // Optionally, split the string to validate each part is in a valid range
        let parts: Vec<&str> = timestamp.split(&['-', ' ', ':'][..]).collect();
        let year: i32 = parts[0].parse().unwrap();
        let month: u32 = parts[1].parse().unwrap();
        let day: u32 = parts[2].parse().unwrap();
        let hour: u32 = parts[3].parse().unwrap();
        let minute: u32 = parts[4].parse().unwrap();
        let second: u32 = parts[5].parse().unwrap();

        // Check if each component of the timestamp is within a valid range
        assert!(year > 1970, "Year is out of range");
        assert!(month >= 1 && month <= 12, "Month is out of range");
        assert!(day >= 1 && day <= 31, "Day is out of range");
        assert!(hour < 24, "Hour is out of range");
        assert!(minute < 60, "Minute is out of range");
        assert!(second < 60, "Second is out of range");
    }
}