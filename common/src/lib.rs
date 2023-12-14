pub mod chess_utils;

use std::fmt;

use serde::{Serialize, Deserialize};
use tokio::io::{self, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};


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

