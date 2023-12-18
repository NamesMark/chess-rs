pub mod chess_utils;

use std::fmt;

use serde::{Serialize, Deserialize};
use tokio::io::{self, AsyncReadExt};
use tokio::net::tcp::OwnedReadHalf;
use log::{info, error};
use thiserror::Error;

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

#[derive(Error, Debug)]
pub enum ChessError {
    #[error("I/O error: {main}, additional info: {context}")]
    IoError {
        main: io::Error,
        context: String,
    },

    #[error("network error: {0}")]
    NetworkError(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("deserialization error: {0}")]
    DeserializationError(String),

    #[error("message handling error: {0}")]
    MessageHandlingError(String),

    #[error("user authentication error: {0}")]
    AuthenticationError(String),

    #[error("database error: {0}")]
    DatabaseError(String),

    #[error("game state error: {0}")]
    GameStateError(String),

    #[error("user state error: {0}")]
    UserStateError(String),
    
    #[error("user not found")]
    UserNotFoundError,


    #[error("sender not found for socket address: {0}")]
    SenderNotFoundError(String),

    #[error("unknown error")]
    Unknown,
}

pub fn make_io_error(e: io::Error, info: &str) -> ChessError {
    ChessError::IoError {
        main: e,
        context: info.to_string(),
    }
}

pub async fn listen_to_messages(reader: &mut OwnedReadHalf) -> Result<Message, ChessError> {
    loop {
        let mut len_bytes = [0u8; 4];

        reader.read_exact(&mut len_bytes).await
            .map_err(|e| make_io_error(e, "Failed to read message length"))?;

        let len = u32::from_be_bytes(len_bytes) as usize;
        //info!("Message length received: {}", len);

        if len > 10 * 1024 * 1024 { 
            return Err(ChessError::MessageHandlingError("Message length too large".to_string()));
        }

        let mut buffer = vec![0u8; len];
        //info!("Buffer allocated with length: {}", buffer.len());

        reader.read_exact(&mut buffer).await
            .map_err(|e| make_io_error(e, "Failed to read message body"))?;

        //info!("Message received, length: {}", buffer.len());
        match serde_cbor::from_slice(&buffer) {
            Ok(message) => {
                info!("Received message (len {}): {:?}", buffer.len(), message);
                return Ok(message)
            }
            Err(e) => {
                error!("Raw data: {:?}", buffer);
                return Err(ChessError::DeserializationError(e.to_string()))
            }
        }
    }
}