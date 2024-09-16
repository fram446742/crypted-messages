mod client;
mod server;
mod tools;
use local_ip_address::local_ip;
use tokio::{self};
use tools::{get_ip, get_port, get_user_input};

#[tokio::main]
async fn main() {
    println!("Welcome to the chat application!");
    if let Err(e) = run().await {
        eprintln!("Application error: {:?}", e);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        // Main menu
        println!("Would you like to run the server or the client?");
        println!("1. Server");
        println!("2. Client");
        println!("3. Show IP address");
        println!("4. Exit");

        match get_user_input(Some("Enter your choice: ")).trim() {
            "1" => {
                if let Err(e) = start_server_flow().await {
                    eprintln!("Server error: {:?}", e);
                }
            }
            "2" => {
                if let Err(e) = start_client().await {
                    eprintln!("Client error: {:?}", e);
                }
            }
            "3" => show_ip_address(),
            "4" => {
                println!("Exiting...");
                return Ok(());
            }
            _ => println!("Invalid choice, please try again."),
        }
    }
}

async fn start_server_flow() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting server...");

    // Ask for the IP address and port to bind the server to
    let ip = get_ip(None, Some("Enter the IP address (leave blank if unsure): "))?;
    let port = get_port(
        None,
        Some("Enter the port to bind the server to (leave blank for 5555): "),
    )?;

    // Start the server
    server::main_server(None, Some(ip), Some(port)).await?;

    println!("Server stopped. Returning to the main menu...");

    Ok(())
}

async fn start_client() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match client::main_client().await {
        Ok(_) => println!("Client session ended. Returning to the main menu..."),
        Err(err) => return Err(err),
    };
    Ok(())
}

fn show_ip_address() {
    match local_ip() {
        Ok(ip) => println!("This is my local IP address: {:?}", ip),
        Err(e) => println!("Error getting local IP: {:?}", e),
    }
}
