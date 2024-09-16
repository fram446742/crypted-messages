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
            "1" => start_server_flow().await?,
            "2" => start_client().await?,
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
    // println!("Do you want to run the server in the background and start client to chat?");
    // println!("1. Yes");
    // println!("2. No");

    // let user_input = get_user_input(Some("Enter your choice: "));
    // let binding = user_input.trim();

    // if binding == "1" {
    //     println!("Running server in the background...");

    //     // Spawn the server in a background task
    //     let server_handle = tokio::spawn(async {
    //         if let Err(e) = server::main_server(None, None).await {
    //             eprintln!("Server encountered an error: {:?}", e);
    //         }
    //     });

    //     println!("Server is running in the background. Starting client...");

    //     // Start client immediately after spawning the server
    //     start_client().await?;

    //     // Optional: You can await the server handle if you want to wait for the server to finish before exiting.
    //     match server_handle.await {
    //         Ok(result) => result,
    //         Err(e) => return Err(Box::new(e)),
    //     }
    // } else {
    // Run the server in the current task (blocking operation)

    // Ask for the IP address and port to bind the server to

    let ip = get_ip(None, Some("Enter the IP address (leave blank if unsure): "))?;
    let port = get_port(
        None,
        Some("Enter the port to bind the server to (leave blank for 5555): "),
    )?;

    // Start the server

    server::main_server(None, Some(ip), Some(port)).await?;
    // }

    Ok(())
}

async fn start_client() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match client::main_client().await {
        Ok(it) => it,
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
