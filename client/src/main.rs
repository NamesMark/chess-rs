#[macro_use]
extern crate lazy_static;
extern crate regex;

use std::io::{self, Write};

use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use log::{info, error};
use regex::Regex;

use common::{Message, Command, DEFAULT_HOST, DEFAULT_PORT, ChessError, listen_to_messages};
use common::chess_utils::{print_board, piece_to_unicode, board_from_string};

lazy_static! {
    static ref LONG_SAN_MOVE_RE: Regex = Regex::new(r"[a-h][1-8][a-h][1-8]").unwrap();
    static ref SAN_MOVE_RE: Regex = Regex::new(
        r"(?x)
        (
            ([RNBQK])?                    # Optional piece indicator (Rook, kNight, Bishop, Queen, King)
            ([a-h1-8])?                   # Optional file or rank specifier for disambiguation
            (x)?                          # Optional capture indicator
            ([a-h][1-8])                  # Destination square
            (=[RNBQ])?                    # Optional promotion indicator
            ([+#])?                       # Optional check/checkmate indicator
        )
        | (O-O(-O)?)                     # Castling (Kingside or Queenside)
        ").unwrap();
}

struct GameState {
    my_username: String,
    //my_elo: u32, // not used for now
    //in_game: bool, // not used for now
    //my_turn: bool, // not used for now
    opponent_username: String,
}

impl GameState {
    fn new() -> Self {
        Self {
            my_username: "".to_string(),
            //in_game: false,
            //my_turn: false,
            opponent_username: "opponent".to_string(),
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
                let _  = get_input(&mut writer).await;
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

async fn get_input(writer: &mut OwnedWriteHalf) -> Result<(), ChessError> {
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
            println!("Available commands: \n`/help` - see this message \n`/log in %username%` - attempt to log in with your username (without percent symbols) \n`/play` - start a chess game \n`/stats` - view your statistics \n`/concede` - give up on the game (your opponent wins) \n`:` - start your message with a semicolon to send a chat message to your opponent\n`e2e4` - send your chess move in long algebraic notation. `O-O` or `O-O-O` for castle.");          
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
            } else if trimmed.starts_with("/concede") {
                Message::Command(Command::Concede)
            } else {
                println!("Unrecognized command. Please use /help to see the list of available commands.");
                continue;
            }

        } else if trimmed.starts_with(":") {
            Message::Text(trimmed[1..].to_string())
        } else if LONG_SAN_MOVE_RE.is_match(trimmed) || SAN_MOVE_RE.is_match(trimmed) {
            Message::Move(trimmed.to_string())
        } else {
            println!("Please enter a valid chess move in algebraic notation, e.g. `e2e4`");
            continue;
        };

        match send_message(writer, &message).await {
            Ok(()) => info!("Message {:?} sent successfully!", message),
            Err(e) => {
                error!("Failed to send message: {}", e);
                return Err(e);
            },
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

async fn send_message(writer: &mut OwnedWriteHalf, message: &Message) -> Result<(), ChessError> {
    let serialized_message = serde_cbor::to_vec(message)
        .map_err(|e| ChessError::IoError { 
            main: io::Error::new(io::ErrorKind::Other, e),
            context: "Failed to serialize message".to_string(),
        })?;
    let len = serialized_message.len() as u32;
    let len_bytes = len.to_be_bytes();

    writer.write_all(&len_bytes).await.map_err(|e| ChessError::IoError {
        main: e,
        context: "Failed to send message length".to_string(),
    })?;
    writer.write_all(&serialized_message).await.map_err(|e| ChessError::IoError {
        main: e,
        context: "Failed to send message".to_string(),
    })?;

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