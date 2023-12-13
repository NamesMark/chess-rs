use std::io::{self, Write};

use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use log::{info, error};

use common::{Message, DEFAULT_HOST, DEFAULT_PORT, Command};

#[tokio::main]
async fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let host = args.get(1).unwrap_or(&DEFAULT_HOST.to_string()).to_string();
    let port = args.get(2).unwrap_or(&DEFAULT_PORT.to_string()).to_string();

    start_client(&host, &port).await;
}

async fn start_client(host: &str, port: &str) {
    match tokio::net::TcpStream::connect(format!("{}:{}", host, port)).await {
        Ok(mut stream) => {
            info!("Successfully connected to server in port {}", port);
            get_input(&mut stream).await;
        }
        Err(e) => {
            error!("Failed to connect: {}", e);
        }
    }
}

async fn get_input(stream: &mut tokio::net::TcpStream) {
    println!("Please enter your command, chat message, or chess move.");
    
    loop {
        print!("> ");
        if let Err(e) = io::stdout().flush() {
            error!("Failed to flush stdout: {}", e);
            continue;
        }

        let mut line = String::new();

        if let Err(e) = io::stdin().read_line(&mut line) {
            error!("Failed to read line: {}", e);
            continue;
        }

        let trimmed = line.trim();
        let message = if trimmed.starts_with("/") {
            if (trimmed.starts_with("/log")) {

                Message::Command(Command::LogIn(("default".to_string()))) // !TODO proper username
            } else if (trimmed.starts_with("/play")) {
                Message::Command(Command::Play)
            } else if (trimmed.starts_with("/stat")) {
                Message::Command(Command::Stats)
            } else {
                error!("Unrecognized command. Please use one of the following: /log in, /play, /stats");
                continue;
            }

        } else if trimmed.starts_with(":") {
            Message::Text(trimmed[1..].to_string())
        } else {
            Message::Move(trimmed.to_string())
        };

        match send_message(stream, &message).await {
            Ok(()) => info!("Message sent successfully!"),
            Err(e) => error!("Failed to send message: {}", e),
        }
        
    }
}

async fn send_message(stream: &mut TcpStream, message: &Message) -> io::Result<()> {
    let serialized_message = serde_cbor::to_vec(&message)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let len = serialized_message.len() as u32;
    let len_bytes = len.to_be_bytes();

    stream.write_all(&len_bytes).await?; 
    stream.write_all(&serialized_message).await?;

    Ok(())
}