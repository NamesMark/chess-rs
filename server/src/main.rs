//use std::process::Command;

mod chess_game;

use std::time::{SystemTime, UNIX_EPOCH};
use std::path::PathBuf;
use std::sync::{Arc};
use std::collections::HashMap;
use std::io::Write;
use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::{OwnedReadHalf};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::fs;
use tokio::signal;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::Sender;
use log::{info, error};
use chess::Board;

use crate::chess_game::{Game};

use common::{DEFAULT_HOST, DEFAULT_PORT, Message, Command, listen_to_messages};
use common::chess_utils::{print_board, piece_to_unicode};

const USER_FILE: &str = "database/usernames.txt";

struct ServerState {
    user_connections: Arc<Mutex<HashMap<String, mpsc::Sender<Message>>>>, // mapping to know the channel through which to send messages to a user 
    anon_user_connections: Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<Message>>>>, // kill me
    addr_to_user: Arc<Mutex<HashMap<SocketAddr, String>>>, // reverse mapping to identify which user the message is coming from
    games: Arc<Mutex<HashMap<String, Game>>>, 
    user_file_mutex: Arc<Mutex<Option<tokio::fs::File>>>,
}

impl ServerState {
    async fn new() -> Self {
        let file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(USER_FILE)
            .await
            .expect("Failed to open user file");

        Self {
            user_connections: Arc::new(Mutex::new(HashMap::new())),
            anon_user_connections: Arc::new(Mutex::new(HashMap::new())),
            addr_to_user: Arc::new(Mutex::new(HashMap::new())),
            games: Arc::new(Mutex::new(HashMap::new())),
            user_file_mutex: Arc::new(Mutex::new(Some(file))),
        }
    }

    // TODO user connections, game session assignments, etc etc
}

struct Connection {
    tx: mpsc::Sender<Message>, // Sender channel
}

impl Connection {
    async fn send_message(&self, message: Message) {
        self.tx.send(message).await.unwrap();
    }
}



#[tokio::main]
async fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let host = args.get(1).unwrap_or(&DEFAULT_HOST.to_string()).to_string();
    let port = args.get(2).unwrap_or(&DEFAULT_PORT.to_string()).to_string();

    info!("Starting server on {}:{}", host, port);

    let (shutdown_sender, _) = broadcast::channel(1);
    let server = tokio::spawn(start_server(host.clone(), port.clone(), shutdown_sender.subscribe()));

    info!("Server spawned, waiting for connections...");

    let ctrl_c = async {
        signal::ctrl_c().await.expect("Failed to listen for ctrl_c signal");
        info!("Ctrl+C signal received, sending shutdown signal...");
        shutdown_sender.send(()).expect("Failed to send shutdown signal");
    };

    tokio::select! {
        _ = server => info!("Server task completed."),
        _ = ctrl_c => info!("Ctrl+C handler completed."),
    }

    info!("Server shutting down.");
}

async fn start_server(host: String, port: String, mut shutdown_signal: broadcast::Receiver<()>) {
    let listener = TcpListener::bind(format!("{}:{}", host, port))
        .await
        .expect("Failed to bind to port");
    info!("Server listening on {}:{}", host, port);

    let server_state = ServerState::new().await;
    let server_state = Arc::new(server_state);

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
    let (tx, mut rx) = mpsc::channel::<Message>(100); // Channel for communication
    let socket_addr = match socket.peer_addr() {
        Ok(addr) => addr,
        Err(e) => {
            error!("Failed to get user address: {}", e);
            return;
        }
    };

    server_state.anon_user_connections.lock().await.insert(socket_addr, tx);
    info!("New anon_user_connections entry added, address: {}", socket_addr);
    //debugging:
    {
        let anon_connections = server_state.anon_user_connections.lock().await;
        info!("Current anon_user_connections: {:?}", anon_connections.keys());
    }

    let (mut reader, mut writer) = socket.into_split();

    let server_state_clone = Arc::clone(&server_state);
    
    let read_task = tokio::spawn(async move {
        listen_to_client_messages(&mut reader, &socket_addr, server_state_clone).await;
    });

    let write_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let serialized_message = match serde_cbor::to_vec(&message) {
                Ok(m) => m,
                Err(e) => {
                    error!("Failed to serialize message: {}", e);
                    continue;
                }
            };

            let len_bytes = (serialized_message.len() as u32).to_be_bytes();
            if writer.write_all(&len_bytes).await.is_err() {
                break;
            }
            if writer.write_all(&serialized_message).await.is_err() {
                break;
            }
        }
    });

    let _ = tokio::try_join!(read_task, write_task);

    {
        let mut anon_user_connections = server_state.anon_user_connections.lock().await;
        if let Some(sender) = anon_user_connections.get(&socket_addr) {
            let _ = sender.send(Message::Log("You have been disconnected. Bye!".to_string())).await;
        }
        anon_user_connections.remove(&socket_addr);
    }
    
    {
        let username = identify_user_by_addr(&socket_addr, server_state.clone()).await;
        let mut user_connections = server_state.user_connections.lock().await;
        if let Some(username) = username {
            if let Some(sender) = user_connections.get(&username) {
                let _ = sender.send(Message::Log("You have been disconnected. Bye!".to_string())).await;
            }
            user_connections.remove(&username);
        }
    }

    
}

async fn listen_to_client_messages(reader: &mut OwnedReadHalf, socket_addr: &SocketAddr, server_state: Arc<ServerState>) {
    loop {
        match listen_to_messages(reader).await {
            Ok(message) => process_message(message, socket_addr, server_state.clone()).await,
            Err(e) => {
                error!("Error while listening to messages: {}", e);
            }
        }
    }
}

async fn process_message(message: Message, socket_addr: &SocketAddr, server_state: Arc<ServerState>) {
    match message {
        Message::Command(command) => process_command(command, socket_addr, server_state).await,
        Message::Move(user_move) => {
            if let Some(username) = identify_user_by_addr(socket_addr, server_state.clone()).await {
                process_move(user_move, username, server_state.clone()).await;
            } else {
                error!("User not found");
            }
        },
        Message::Text(text) => {
            info!("Received the following text message: {}", text)
            // find game
            // relay chat message to the user's opponent
        },
        Message::Board(board_string) => panic!("Expected Command, Move or Text, received Board"),
        Message::Error(e) => {},
        Message::Log(message) => {},
    }
}

async fn identify_user_by_addr(socket_addr: &SocketAddr, server_state: Arc<ServerState>) -> Option<String> {
    return server_state.addr_to_user.lock().await.get(&socket_addr).cloned()
}

async fn process_command(command: Command, socket_addr: &SocketAddr, server_state: Arc<ServerState>) {
    match command {
        Command::LogIn(username) => {
            if authenticate(&username).await {
                let sender = {
                    let mut anon_connections = server_state.anon_user_connections.lock().await;
                    anon_connections.remove(&socket_addr)
                };
                if let Some(sender) = sender {
                    let mut user_connections = server_state.user_connections.lock().await;
                    user_connections.insert(username.clone(), sender.clone());
                    let mut addr_user = server_state.addr_to_user.lock().await;
                    addr_user.insert(socket_addr.clone(), username.clone());
                    send_message(&username, Message::Log(format!("Authenticated successfully. Welcome back, {}.", username)), server_state.clone(), &sender).await;
                } else {
                    error!("Sender not found for socket address: {:?}", socket_addr);
                    //debugging:
                    {
                        let anon_connections = server_state.anon_user_connections.lock().await;
                        info!("Current anon_user_connections: {:?}", anon_connections.keys());
                    }

                }
            } else {
                register(&username).await;
                let sender = {
                    let mut anon_connections = server_state.anon_user_connections.lock().await;
                    anon_connections.remove(&socket_addr)
                };
                if let Some(test_sender) = sender.clone() {
                    info!("Sender is {:?}", test_sender);
                } else {
                    info!("Sender doesn't exist");
                }
                if let Some(sender) = sender {
                    info!("Trying to insert the user into user_connections");
                    let mut user_connections = server_state.user_connections.lock().await;
                    user_connections.insert(username.clone(), sender.clone());
                    let mut addr_user = server_state.addr_to_user.lock().await;
                    addr_user.insert(socket_addr.clone(), username.clone());
                    //debugging:
                    {
                        info!("Current user_connections: {:?}", user_connections.keys());
                    }
                    send_message(&username, Message::Log(format!("Registered a new user. Welcome, {}! Hope you are going to enjoy our chess server. Use /play to start your first game!", username)), server_state.clone(), &sender).await;
                } else {
                    error!("Tried registering. Sender not found for socket address: {:?}", socket_addr);
                    //debugging:
                    {
                        let anon_connections = server_state.anon_user_connections.lock().await;
                        info!("Current anon_user_connections: {:?}", anon_connections.keys());
                    }
                }
            }
        }, 
        Command::Play => {
            if let Some(username) = identify_user_by_addr(&socket_addr, server_state.clone()).await {
                // TODO check if the player is already in game
                assign_to_game(username, server_state.clone()).await;
            } else {
                error!("Failed to get username from the server state (unregistered player tried to play).");
                //send_message(&username, Message::Error("Anonymous users cannot start games. Please use /log in.".to_string()), server_state).await; //I'll handle it on the client
            }

        },
        Command::Concede => {},
        Command::Stats => {},
        _ => unreachable!("Unexpected command {command}")
    }
}

async fn authenticate(username: &str) -> bool {
    info!("Trying to authenticate {username}...");
    match tokio::fs::read_to_string(USER_FILE)
    .await {
        Ok(file_contents) => {
            let result = file_contents.lines().any(|line| line == username);
            info!("Checked the file, found {username}: {result}");
            result
        },
        Err(e) => {
            error!("Failed to open user file: {}", e);
            false
        }
    }
}

async fn register(username: &str) {
    info!("Trying to register {username}...");
    match tokio::fs::OpenOptions::new()
        .append(true)
        .open(USER_FILE)
        .await {
            Ok(mut file) => {
                let content = format!("{}\n", username);
                if let Err(e) = file.write_all(content.as_bytes()).await {
                    error!("Failed to write to user file: {}", e);
                } else {
                    info!("Registered {username}.");
                }
                
            }
            Err(e) => error!("Failed to open user file for writing: {}", e),
    }
}

async fn process_move(user_move: String, username: String, server_state: Arc<ServerState>) {
    let mut games = server_state.games.lock().await;
    if let Some(game) = games.get_mut(&username) {
        match game.make_move(&user_move) {
            Ok(_) => info!("Move made: {}", user_move),
            Err(err_msg) => error!("{}", err_msg),
        }
    }
}

async fn start_game(username: String, server_state: Arc<ServerState>) {
    let mut games = server_state.games.lock().await;
    games.entry(username.clone()).or_insert_with(Game::new);

    if let Some(game) = games.get(&username) {

        todo!("Init the board, etc.");
        info!("Starting game for user: {}", username);
    }

    todo!("Send initial game state to both players");
}

async fn assign_to_game(username: String, server_state: Arc<ServerState>) {
    let mut games = server_state.games.lock().await;
    let mut user_game_assigned = false;

    // Find an existing game with a player slot open
    for game in games.values_mut() {
        if game.white.is_none() {
            game.white = Some(username.clone());
            user_game_assigned = true;
            info!("{username} is now white in a game");
            break;
        } else if game.black.is_none() {
            game.black = Some(username.clone());
            user_game_assigned = true;
            info!("{username} is now black in a game");
            break;
        }
    }

    // If no open games, create a new one
    if !user_game_assigned {
        let new_game = Game {
            board: Board::default(),
            current_turn: chess::Color::White,
            white: Some(username.clone()),
            black: None,
            status: chess_game::GameStatus::Pending,
        };
        games.insert(username.clone(), new_game);
        info!("New game created for {username}");
    }

    if let Some(sender) = server_state.user_connections.lock().await.get(&username) {
        send_message(&username, Message::Log(format!("You're in a game now!")), server_state.clone(), sender).await;
    }
}

async fn send_message(username: &str, message: Message, server_state: Arc<ServerState>, sender: &Sender<Message>) {
    info!("Trying to send message {:?} to {username}", message);
    // let sender = {
    //         let user_connections = server_state.user_connections.lock().await;
    //         user_connections.get(username).cloned()
    //     };

    //debugging:
    {
        info!("Sender is {:?}", sender);
    }

    if let Err(e) = sender.send(message).await {
        error!("Failed to send message to {}: {}", username, e);
    }
    info!("Successfully sent message to {username}");
}