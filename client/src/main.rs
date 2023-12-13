use std::io::{self, Write};

use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use log::{info, error};

use common::{Message, Command, DEFAULT_HOST, DEFAULT_PORT, send_message};
use common::chess_utils::{print_board, piece_to_unicode};

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



async fn process_message(message: Message) {
    match message {
        Message::Command(command) => panic!("Expected Board, Text, Log, received Command"),
        Message::Move(user_move) => panic!("Expected Board, Text, Log, received Move"),
        Message::Text(text) => {},
        Message::Board(board_string) => {},
        Message::Error(e) => {},
        Message::Log(message) => {},
    }
}