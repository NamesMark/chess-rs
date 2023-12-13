use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::PathBuf;

use tokio::net::{TcpListener, TcpStream};
use tokio::io::AsyncReadExt;
use tokio::fs;
use tokio::signal;
use tokio::sync::broadcast;
use log::{info, error};

use common::{DEFAULT_HOST, DEFAULT_PORT, Message, Command, print_board, piece_to_unicode};

#[tokio::main]
async fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let host = args.get(1).unwrap_or(&DEFAULT_HOST.to_string()).to_string();
    let port = args.get(2).unwrap_or(&DEFAULT_PORT.to_string()).to_string();


    let (shutdown_sender, _) = broadcast::channel(1);
    let server = tokio::spawn(start_server(host.clone(), port.clone(), shutdown_sender.subscribe()));

    let ctrl_c = async {
        signal::ctrl_c().await.expect("Failed to listen for ctrl_c signal");
        shutdown_sender.send(()).expect("Failed to send shutdown signal");
    };

    tokio::select! {
        _ = server => {},
        _ = ctrl_c => {},
    }

    info!("Server shutting down.");
}

async fn start_server(host: String, port: String, mut shutdown_signal: broadcast::Receiver<()>) {
    let listener = TcpListener::bind(format!("{}:{}", host, port))
        .await
        .expect("Failed to bind to port");
    info!("Server listening on {}:{}", host, port);

    loop {
        tokio::select! {
            Ok((socket, _)) = listener.accept() => {
                tokio::spawn(async move {
                    info!("New connection: {}", socket.peer_addr().unwrap());
                    handle_client(socket).await;
                });
            }
            _ = shutdown_signal.recv() => {
                info!("Shutdown signal received.");
                break;
            }
        }
    }
}

async fn handle_client(mut socket: TcpStream) {
    let mut len_bytes = [0u8; 4];
    if let Err(e) = socket.read_exact(&mut len_bytes).await {
        error!("Failed to read message length: {}", e);
        return;
    }
    let len = u32::from_be_bytes(len_bytes) as usize;
    info!("Message length received: {}", len);

    if len > 10 * 1024 * 1024 { 
        error!("Message length too large: {}", len);
        return;
    }

    let mut buffer = vec![0u8; len];
    info!("Buffer allocated with length: {}", buffer.len());
    match socket.read_exact(&mut buffer).await {
        Ok(_) => {
            info!("Message received, length: {}", buffer.len());
            match serde_cbor::from_slice(&buffer) {
                Ok(message) => {
                    info!("Received message: {:?}", message);
                    process_message(message).await;
                }
                Err(e) => {
                    error!("Deserialization error: {}", e);
                    error!("Raw data: {:?}", buffer);
                }
            }
        }
        Err(e) => {
            error!("Failed to read message: {}", e);
        }
    }
}

async fn process_message(message: Message) {
    match message {
        Message::Command(command) => process_command(command),
        Message::Move(user_move) => process_move(user_move),
        Message::Text(text) => println!("Received the following text message: {}", text),
        Message::Board(board_string) => panic!("Expected Command, Move or Text, received Board"),
        Message::Error(e) => {},
        Message::Log(message) => {},
    }
}

async fn process_command(command: Command) {
    match command {
        LogIn(username) => log_in(username), 
        Play => insert_player,
        Concede => {},
        Stats => {},
        _ => panic!("Unexpected command")
    }
}

async fn process_move(user_move: String) {
    
}

