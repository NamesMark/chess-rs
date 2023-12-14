#[macro_use]
extern crate lazy_static;
extern crate regex;

use std::io::{self, Write};

use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use log::{info, error};
use regex::Regex;

use common::{Message, Command, DEFAULT_HOST, DEFAULT_PORT, listen_to_messages};
use common::chess_utils::{print_board, piece_to_unicode, board_from_string};

lazy_static! {
    static ref MOVE_RE: Regex = Regex::new(r"[a-h][1-8][a-h][1-8]").unwrap();
}

struct GameState {
    my_username: String,
    in_game: bool,
    my_turn: bool,
    opponent_username: String,
}

impl GameState {
    fn new() -> Self {
        Self {
            my_username: "".to_string(),
            in_game: false,
            my_turn: false,
            opponent_username: "".to_string(),
        }
    }
}

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
        Ok(stream) => {
            info!("Successfully connected to server in port {}", port);

            let game_state = GameState::new();

            let (mut reader, mut writer) = stream.into_split();
            let read_task = tokio::spawn(async move {
                listen_to_server_messages(&mut reader, &game_state).await;
            });

            let write_task = tokio::spawn(async move {
                get_input(&mut writer).await;
            });

            tokio::try_join!(read_task, write_task).unwrap();
        }
        Err(e) => {
            error!("Failed to connect: {}", e);
        }
    }
}

async fn listen_to_server_messages(reader: &mut OwnedReadHalf, game_state: &GameState) {
    loop {
        match listen_to_messages(reader).await {
            Ok(message) => process_message(message, game_state).await,
            Err(e) => {
                error!("Error while listening to messages: {}", e);
                break;
            }
        }
    }
}

async fn get_input(writer: &mut OwnedWriteHalf) {
    println!("Please enter your command, chat message, or chess move.");
    
    loop {
        print!("> ");
        if let Err(e) = std::io::stdout().flush() {
            error!("Failed to flush stdout: {}", e);
            continue;
        }

        let mut line = String::new();

        if let Err(e) = std::io::stdin().read_line(&mut line) {
            error!("Failed to read line: {}", e);
            continue;
        }

        let trimmed = line.trim();

        if trimmed.starts_with("/help") {
            println!("Available commands: \n//help - see this message \n//log in username - attempt to log in \n//play - start a chess game \n//stats - view your statistics \n//concede - give up on the game (your opponent wins) \n: - start with semicolon to send a chat message \ne2e4 - send your chess move in long algebraic notation");          
            continue;
        }

        let message = if trimmed.starts_with("/") {
            if trimmed.starts_with("/log") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() != 3 {
                    println!("Please log in with you username like this: /log in your_username.");
                    continue;
                }
                let username = parts[parts.len()-1];
                Message::Command(Command::LogIn(username.to_string())) 
            } else if trimmed.starts_with("/play") {
                Message::Command(Command::Play)
            } else if trimmed.starts_with("/stat") {
                Message::Command(Command::Stats)
            } else {
                println!("Unrecognized command. Please use /help to see the list of available commands.");
                continue;
            }

        } else if trimmed.starts_with(":") {
            Message::Text(trimmed[1..].to_string())
        } else if MOVE_RE.is_match(trimmed)  {
            Message::Move(trimmed.to_string())
        } else {
            println!("Please enter a valid chess move in algebraic notation, e.g. `e2e4`");
            continue;
        };

        match send_message(writer, &message).await {
            Ok(()) => info!("Message {:?} sent successfully!", message),
            Err(e) => error!("Failed to send message: {}", e),
        }
        
    }
}



async fn process_message(message: Message, game_state: &GameState) {
    match message {
        Message::Command(command) => panic!("Expected Board, Text, Log, received Command"),
        Message::Move(user_move) => panic!("Expected Board, Text, Log, received Move"),
        Message::Text(text) => display_chat_message(text, game_state),
        Message::Board(board_string) => display_board(board_string),
        Message::Error(e) => display_error_message(e),
        Message::Log(message) => display_log_message(message),
    }
}

async fn send_message(writer: &mut OwnedWriteHalf, message: &Message) -> io::Result<()> {
    let serialized_message = serde_cbor::to_vec(&message)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let len = serialized_message.len() as u32;
    let len_bytes = len.to_be_bytes();

    writer.write_all(&len_bytes).await?; 
    writer.write_all(&serialized_message).await?;

    Ok(())
}

fn display_board(board_string: String) {
    print_board(&board_from_string(board_string).expect("Unexpected board format."));
}

fn display_log_message(message: String) {
    println!("[SERVER] {message}");
}

fn display_error_message(message: String) {
    println!("[SERVER ERROR] {message}");
}

fn display_chat_message(message: String, game_state: &GameState) {
    let opponent = &game_state.opponent_username;
    println!("[{opponent}]: {message}");
}