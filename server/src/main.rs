//use std::process::Command;

mod chess;

use std::time::{SystemTime, UNIX_EPOCH};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::io::Write;

use tokio::net::{TcpListener, TcpStream};
use tokio::io::{self, AsyncReadExt};
use tokio::fs;
use tokio::signal;
use tokio::sync::broadcast;
use log::{info, error};

use common::{DEFAULT_HOST, DEFAULT_PORT, Message, Command, send_message};
use common::chess_utils::{print_board, piece_to_unicode};

const USER_FILE: String = "database/usernames.txt".to_string();

struct ServerState {
    user_connections: Arc<Mutex<HashMap<String, TcpStream>>>, // Thread-safe mapping between tcp connections and usernames 
    games: Arc<Mutex<HashMap<String, GameSession>>>, 
}

#[derive(Debug)]
struct Game {
    white: Option<String>,
    black: Option<String>,
}

impl ServerState {
    fn new() -> Self {
        Self {
            user_connections: Arc::new(Mutex::new(HashMap::new())),
            games: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // TODO user connections, game session assignments, etc etc
}


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

    let server_state = Arc::new(ServerState::new());

    loop {
        tokio::select! {
            Ok((socket, _)) = listener.accept() => {
                let server_state_clone = server_state.clone();
                tokio::spawn(async move {
                    info!("New connection: {}", socket.peer_addr().unwrap());
                    handle_client(socket, server_state_clone).await;
                });
            }
            _ = shutdown_signal.recv() => {
                info!("Shutdown signal received.");
                break;
            }
        }
    }
}

async fn handle_client(mut socket: TcpStream, server_state: Arc<ServerState>) {
    let mut len_bytes = [0u8; 4];

    loop {
        
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
                        process_message(message, &mut socket, server_state.clone()).await;
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

async fn process_message(message: Message, socket: &mut TcpStream, server_state: Arc<ServerState>) {
    match message {
        Message::Command(command) => process_command(command, socket, server_state).await,
        Message::Move(user_move) => process_move(user_move, socket, server_state).await,
        Message::Text(text) => println!("Received the following text message: {}", text),
        Message::Board(board_string) => panic!("Expected Command, Move or Text, received Board"),
        Message::Error(e) => {},
        Message::Log(message) => {},
    }
}

async fn process_command(command: Command, socket: &mut TcpStream, server_state: Arc<ServerState>) {
    match command {
        Command::LogIn(username) => {
            if authenticate(&username).await {
                server_state.user_connections.lock().unwrap().insert(username.clone(), socket.try_clone().unwrap());
                send_message(socket, &Message::Log(format!("Authenticated successfully. Welcome back, {}.", username))).await.unwrap();
            } else {
                register(&username);
                server_state.user_connections.lock().unwrap().insert(username.clone(), socket.try_clone().unwrap());
                send_message(socket, &Message::Log(format!("Registered a new user. Welcome, {}! Hope you are going to enjoy our chess server. Use /play to start your first game!", username))).await.unwrap();
            }
        }, 
        Command::Play => // how do I know the username at this point? I guess needed to add another HashMap to do a reverse lookup
        todo!()
         assign_to_game(&username, server_state.clone()).await,
        Command::Concede => {},
        Command::Stats => {},
        _ => unreachable!("Unexpected command {command}")
    }
}

async fn authenticate(username: &str) -> bool {
    match std::fs::read_to_string(USER_FILE) {
        Ok(file_contents) => file_contents.lines().any(|line| line == username),
        Err(e) => {
            error!("Failed to open user file: {}", e);
            false
        }
    }
}

fn register(username: &str) {
    match std::fs::OpenOptions::new()
        .append(true)
        .open(USER_FILE) {
            Ok(mut file) => {
                if let Err(e) = writeln!(file, "{}", username) {
                    error!("Failed to write to user file: {}", e);
                }
            }
            Err(e) => error!("Failed to open user file for writing: {}", e),
    }
}

async fn process_move(user_move: String, socket: &mut TcpStream, server_state: Arc<ServerState>) {
    todo!()
}

async fn start_game(username: String, server_state: Arc<ServerState>) {
    let games = server_state.games.lock().unwrap();
    if let Some(game) = games.get(&username) {

        todo!("Init the board, etc.");
        info!("Starting game for user: {}", username);
    }

    todo!("Send initial game state to both players");
}

async fn assign_to_game(username: String, server_state: Arc<ServerState>) {
    let mut games = server_state.games.lock().unwrap();
    let mut user_game_assigned = false;

    // Find an existing game with a player slot open
    for game in games.values_mut() {
        if game.white.is_none() {
            game.white = Some(username.clone());
            user_game_assigned = true;
            break;
        } else if game.black.is_none() {
            game.black = Some(username.clone());
            user_game_assigned = true;
            break;
        }
    }

    // If no open games, create a new one
    if !user_game_assigned {
        let new_game = Game {
            white: Some(username.clone()),
            black: None,
        };
        games.insert(username, new_game);
    }

    if let Some(socket) = server_state.user_connections.lock().unwrap().get_mut(&username) {
        send_message(socket, &Message::Log(format!("You're in a game now!"))).await.unwrap();
    }
}