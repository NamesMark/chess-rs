pub mod chess_utils;

use std::fmt;

use serde::{Serialize, Deserialize};
use tokio::io::{self, AsyncReadExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::{OwnedReadHalf};
use log::{info, error};

pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: &str = "11111";

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    Command(Command), // technical client-server commands 
    Move(String), // chess move in algebraic notation like `e2e4`
    Text(String), // chat messages
    Board(String), // represents chess::Board and is parsed on the client
    Error(String),
    Log(String), // other notifications from the server
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Command {
    LogIn(String), // `/log_in`
    //LogOut,   // `/log_out`
    Play,    // `/play`
    Concede, // `/concede`
    Stats,
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::LogIn(username) => write!(f, "LogIn({})", username),
            Command::Play => write!(f, "Play"),
            Command::Concede => write!(f, "Concede"),
            Command::Stats => write!(f, "Stats"),
            _ => unreachable!("Unexpected new command")
        }
    }
}

pub async fn listen_to_messages(reader: &mut OwnedReadHalf) -> io::Result<Message> {
    loop {
        let mut len_bytes = [0u8; 4];

        if let Err(e) = reader.read_exact(&mut len_bytes).await {
            error!("Failed to read message length: {}", e);
            return Err(e);
        }
        let len = u32::from_be_bytes(len_bytes) as usize;
        info!("Message length received: {}", len);

        if len > 10 * 1024 * 1024 { 
            error!("Message length too large: {}", len);
            return Err(io::Error::new(io::ErrorKind::Other, "Message length too large"));
        }

        let mut buffer = vec![0u8; len];
        info!("Buffer allocated with length: {}", buffer.len());

        match reader.read_exact(&mut buffer).await {
            Ok(_) => {
                info!("Message received, length: {}", buffer.len());
                match serde_cbor::from_slice(&buffer) {
                    Ok(message) => {
                        info!("Received message: {:?}", message);
                        message
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
}