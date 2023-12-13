use serde::{Serialize, Deserialize};

use chess_utils;

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

async fn send_message(stream: &mut TcpStream, message: &Message) -> io::Result<()> {
    let serialized_message = serde_cbor::to_vec(&message)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let len = serialized_message.len() as u32;
    let len_bytes = len.to_be_bytes();

    stream.write_all(&len_bytes).await?; 
    stream.write_all(&serialized_message).await?;

    Ok(())
}

