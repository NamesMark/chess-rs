//use std::process::Command;

mod chess_game;

use std::time::{SystemTime, UNIX_EPOCH};
use std::path::PathBuf;
use std::sync::{Arc};
use std::sync::atomic::{AtomicU32, Ordering};
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
use chess::{Board, Color};

use crate::chess_game::{Game, GameStatus};

use common::{DEFAULT_HOST, DEFAULT_PORT, Message, Command, ChessError, make_io_error, listen_to_messages};
use common::chess_utils::{print_board, piece_to_unicode};

const USER_FILE: &str = "database/usernames.txt";

struct ServerState {
    user_connections: Arc<Mutex<HashMap<String, mpsc::Sender<Message>>>>, // mapping to know the channel through which to send messages to a user 
    anon_user_connections: Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<Message>>>>, // kill me
    addr_to_user: Arc<Mutex<HashMap<SocketAddr, String>>>, // reverse mapping to identify which user the message is coming from
    games: Arc<Mutex<HashMap<u32, Arc<Mutex<Game>>>>>, // game_id to Game
    finished_games: Arc<Mutex<HashMap<u32, Arc<Mutex<Game>>>>>,
    user_to_game: Arc<Mutex<HashMap<String, u32>>>, // username to game_id
    user_file_mutex: Arc<Mutex<Option<tokio::fs::File>>>,
    last_game_id: AtomicU32, 
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
            finished_games: Arc::new(Mutex::new(HashMap::new())),
            user_to_game: Arc::new(Mutex::new(HashMap::new())),
            user_file_mutex: Arc::new(Mutex::new(Some(file))),
            last_game_id: AtomicU32::new(0),
        }
    }

    fn get_new_game_id(&self) -> u32 {
        self.last_game_id.fetch_add(1, Ordering::SeqCst)
    }

    // TODO user connections, game session assignments, etc etc
}

// struct Connection {
//     tx: mpsc::Sender<Message>, // Sender channel
// }

// impl Connection {
//     async fn send_message(&self, message: Message) {
//         self.tx.send(message).await.unwrap();
//     }
// }



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
            let _ = sender.send(Message::Log("You have been disconnected. Bye!".to_string())).await
                .map_err(|e| ChessError::MessageHandlingError(format!("Failed to send message: {}", e)));
        }
        anon_user_connections.remove(&socket_addr);
    }
    
    {
        let username = identify_user_by_addr(&socket_addr, &server_state).await;
        let mut user_connections = server_state.user_connections.lock().await;
        if let Some(username) = username {
            if let Some(sender) = user_connections.get(&username) {
                let _ = sender.send(Message::Log("You have been disconnected. Bye!".to_string())).await
                    .map_err(|e| ChessError::MessageHandlingError(format!("Failed to send message: {}", e)));
            }
            user_connections.remove(&username);
        }
    }

    
}

async fn listen_to_client_messages(reader: &mut OwnedReadHalf, socket_addr: &SocketAddr, server_state: Arc<ServerState>) {
    loop {
        match listen_to_messages(reader).await {
            Ok(message) => {
                match process_message(message, socket_addr, server_state.clone()).await {
                    Ok(_) => {},
                    Err(e) => {
                        error!("Error while processing messages: {}", e);
                    }
                }
            }
            
            Err(e) => {
                error!("Error while listening to messages: {}", e);
                break;
            }
        }
    }
}

async fn process_message(message: Message, socket_addr: &SocketAddr, server_state: Arc<ServerState>) -> Result<(), ChessError> {
    match message {
        Message::Command(command) => process_command(command, socket_addr, server_state).await,
        Message::Move(player_move) => { 
            // TODO: refactor - we identify game twice while processing move
            if let Some(username) = identify_user_by_addr(socket_addr, &server_state).await {
                if let Err(err) = process_move(player_move, &username, &server_state).await {
                    // Handle move processing error (e.g., invalid move)
                    return Err(err);
                }
        
                match identify_game(&username, &server_state).await {
                    Ok(game) => {
                        send_game_state(&mut *game.lock().await, &server_state).await?;
                        Ok(())
                    },
                    Err(err) => {
                        Err(err)
                    }
                }
            } else {
                Err(ChessError::UserNotFoundError)
            }
        },
        Message::Text(text) => {
            info!("Received the following text message: {}", text);
            let username = identify_user_by_addr(socket_addr, &server_state).await
                .ok_or(ChessError::UserStateError("User not found".to_string()))?;
            if let Some(opponent) = identify_opponent(username, &server_state).await? {
                if let Some(sender) = server_state.user_connections.lock().await.get(&opponent) {
                    sender.send(Message::Text(text.clone())).await
                        .map_err(|e| ChessError::MessageHandlingError(format!("Failed to send message: {}", e)))?;
                }
            }
            Ok(())
        },
        Message::Board(board_string) => panic!("Expected Command, Move or Text, received Board"),
        Message::Error(e) => panic!("Expected Command, Move or Text, received Error"),
        Message::Log(message) => panic!("Expected Command, Move or Text, received Log"),
    }
}

async fn send_game_state(game: &mut Game, server_state: &Arc<ServerState>) -> Result<(), ChessError> {
    let user_connections = server_state.user_connections.lock().await;

    let white_player = game.white.as_ref().ok_or(ChessError::GameStateError("White player missing".to_string()))?;
    let black_player = game.black.as_ref().ok_or(ChessError::GameStateError("Black player missing".to_string()))?;

    let board_state = &game.board.to_string();

    let white_sender = user_connections.get(white_player).ok_or(ChessError::UserNotFoundError)?;
    let black_sender = user_connections.get(black_player).ok_or(ChessError::UserNotFoundError)?;

    white_sender.send(Message::Board(board_state.clone())).await.map_err(|e| ChessError::MessageHandlingError(e.to_string()))?;
    black_sender.send(Message::Board(board_state.to_string())).await.map_err(|e| ChessError::MessageHandlingError(e.to_string()))?;

    if game.current_turn == Color::White {
        white_sender.send(Message::Log(format!("Your turn, white player {white_player}!"))).await.map_err(|e| ChessError::MessageHandlingError(e.to_string()))?;
    } else {
        black_sender.send(Message::Log(format!("Your turn, black player {black_player}!"))).await.map_err(|e| ChessError::MessageHandlingError(e.to_string()))?;
    }

    if game.result == None {
        if game.is_check() {
            white_sender.send(Message::Log(format!("Check!"))).await.map_err(|e| ChessError::MessageHandlingError(e.to_string()))?;
            black_sender.send(Message::Log(format!("Check!"))).await.map_err(|e| ChessError::MessageHandlingError(e.to_string()))?;
        }
        return Ok(());
    } 
    
    if game.is_mate() {
        white_sender.send(Message::Log(format!("Mate!"))).await.map_err(|e| ChessError::MessageHandlingError(e.to_string()))?;
        black_sender.send(Message::Log(format!("Mate!"))).await.map_err(|e| ChessError::MessageHandlingError(e.to_string()))?;
    }

    white_sender.send(Message::Log(format!("Game is finished. Result is: {:?}", game.result))).await.map_err(|e| ChessError::MessageHandlingError(e.to_string()))?;
    black_sender.send(Message::Log(format!("Game is finished. Result is: {:?}", game.result))).await.map_err(|e| ChessError::MessageHandlingError(e.to_string()))?; 

    Ok(())
}

async fn identify_game(username: &String, server_state: &Arc<ServerState>) -> Result<Arc<Mutex<Game>>, ChessError> {
    let user_to_game = server_state.user_to_game.lock().await;
    if let Some(&game_id) = user_to_game.get(username) {
        drop(user_to_game); 

        let games = server_state.games.lock().await;
        if let Some(game_arc) = games.get(&game_id) {
            Ok(game_arc.clone())
        } else {
            Err(ChessError::GameStateError("Game not found for user".to_string()))
        }
    } else {
        Err(ChessError::GameStateError("User not in game".to_string()))
    }
}

async fn identify_opponent(username: String, server_state: &Arc<ServerState>) -> Result<Option<String>, ChessError> {
    let user_to_game = server_state.user_to_game.lock().await;
    if let Some(&game_id) = user_to_game.get(&username) {
        drop(user_to_game);

        let games = server_state.games.lock().await;
        if let Some(game_arc) = games.get(&game_id) {
            let game = game_arc.lock().await;
            Ok(if game.black.as_ref() == Some(&username) {
                game.white.clone()
            } else {
                game.black.clone()
            })
        } else {
            Err(ChessError::GameStateError("Game not found for user".to_string()))
        }
    } else {
        Err(ChessError::GameStateError("User not in game".to_string()))
    }
}

async fn identify_user_by_addr(socket_addr: &SocketAddr, server_state: &Arc<ServerState>) -> Option<String> {
    return server_state.addr_to_user.lock().await.get(&socket_addr).cloned()
}

async fn process_command(command: Command, socket_addr: &SocketAddr, server_state: Arc<ServerState>) -> Result<(), ChessError> {
    match command {
        Command::LogIn(username) => {
            if authenticate(&username).await? {
                let sender = {
                    let mut anon_connections = server_state.anon_user_connections.lock().await;
                    anon_connections.remove(&socket_addr)
                };
                if let Some(sender) = sender {
                    let mut user_connections = server_state.user_connections.lock().await;
                    user_connections.insert(username.clone(), sender.clone());
                    let mut addr_user = server_state.addr_to_user.lock().await;
                    addr_user.insert(socket_addr.clone(), username.clone());
                    let _ = send_message(&username, Message::Log(format!("Authenticated successfully. Welcome back, {}.", username)), &sender).await?;
                    Ok(())
                } else {
                    Err(ChessError::SenderNotFoundError(format!("Sender not found for socket address: {:?}", socket_addr)))
                }
            } else {
                let _ = register(&username).await?;
                let sender = {
                    let mut anon_connections = server_state.anon_user_connections.lock().await;
                    anon_connections.remove(&socket_addr)
                };
                if let Some(sender) = sender {
                    info!("Trying to insert the user into user_connections");
                    let mut user_connections = server_state.user_connections.lock().await;
                    user_connections.insert(username.clone(), sender.clone());
                    let mut addr_user = server_state.addr_to_user.lock().await;
                    addr_user.insert(socket_addr.clone(), username.clone());
                    let _ = send_message(&username, Message::Log(format!("Registered a new user. Welcome, {}! Hope you are going to enjoy our chess server. Use /play to start your first game!", username)), &sender).await?;
                    Ok(())
                } else {
                    Err(ChessError::SenderNotFoundError(format!("Tried registering. Sender not found for socket address: {:?}", socket_addr)))
                }
            }
        }, 
        Command::Play => {
            info!("Processing play command");
            if let Some(username) = identify_user_by_addr(&socket_addr, &server_state).await {
                let user_to_game = server_state.user_to_game.lock().await;
                if user_to_game.contains_key(&username) {
                    drop(user_to_game);
                    let user_connections = server_state.user_connections.lock().await;
                    if let Some(sender) = user_connections.get(&username).cloned() {
                        send_message(&username, Message::Error("You cannot start a new game until this one is finished!".to_string()), &sender).await?;
                    }
                    return Err(ChessError::UserStateError("User already in a game.".to_string()));
                }

                drop(user_to_game); 
                info!("Assigning to a game");
                assign_to_game(username, server_state.clone()).await
            } else {
                let sender = {
                    let mut anon_connections = server_state.anon_user_connections.lock().await;
                    anon_connections.remove(&socket_addr)
                };
                if let Some(sender) = sender {
                    let _ = sender.send(Message::Error("Anonymous users cannot start games. Please use /log in.".to_string())).await;
                }
                Err(ChessError::UserStateError("Failed to get username from the server state (unregistered player tried to play).".to_string()))
            }
        },
        Command::Concede => {
            if let Some(username) = identify_user_by_addr(socket_addr, &server_state).await {
                match identify_game(&username, &server_state).await {
                    Ok(game_arc) => {
                        let mut game = game_arc.lock().await;
                        game.concede(&username).unwrap_or_else(|e| error!("Error during concession: {}", e));
        
                        let mut user_to_game = server_state.user_to_game.lock().await;
                        if let Some(player) = game.white.as_ref() {
                            user_to_game.remove(player);
                        }
                        if let Some(player) = game.black.as_ref() {
                            user_to_game.remove(player);
                        }
        
                        send_game_state(&mut game, &server_state).await?;
        
                        Ok(())
                    },
                    Err(err) => Err(err),
                }
            } else {
                Err(ChessError::UserNotFoundError)
            }
        }
        Command::Stats => unimplemented!("TODO stats"),
        _ => unreachable!("Unexpected command {command}")
    }
}

async fn authenticate(username: &str) -> Result<bool, ChessError> {
    info!("Trying to authenticate {username}...");
    match tokio::fs::read_to_string(USER_FILE)
    .await {
        Ok(file_contents) => {
            let result = file_contents.lines().any(|line| line == username);
            info!("Checked the file, found {username}: {result}");
            Ok(result)
        },
        Err(e) => {
            //Err(ChessError::AuthenticationError(format!("Failed to open user file: {}", e)))
            Err(make_io_error(e, "Failed to open user file"))
        }
    }
}

async fn register(username: &str) -> Result<bool, ChessError>  {
    info!("Trying to register {username}...");
    match tokio::fs::OpenOptions::new()
        .append(true)
        .open(USER_FILE)
        .await {
            Ok(mut file) => {
                let content = format!("{}\n", username);
                if let Err(e) = file.write_all(content.as_bytes()).await {
                    Err(make_io_error(e, "Failed to write to user file"))
                } else {
                    info!("Registered {username}.");
                    Ok(true)
                }
                
            },
            Err(e) => {
                Err(make_io_error(e, "Failed to open user file for writing"))
            }
    }
}

async fn process_move(user_move: String, username: &String, server_state: &Arc<ServerState>) -> Result<(), ChessError> {
    let user_to_game = server_state.user_to_game.lock().await;
    if let Some(&game_id) = user_to_game.get(username) {
        drop(user_to_game);

        let games = server_state.games.lock().await;
        if let Some(game_arc) = games.get(&game_id) {
            let mut game = game_arc.lock().await;

            if game.white.is_none() || game.black.is_none() {
                if let Some(sender) = server_state.user_connections.lock().await.get(username) {
                    sender.send(Message::Error("The game has not started yet. We are waiting for a second player to join.".to_string())).await
                        .map_err(|e| ChessError::MessageHandlingError(format!("Failed to send message: {}", e)))?;
                }
                return Err(ChessError::GameStateError("The game has not started yet. We are waiting for a second player to join.".to_string()));
            } else if !(game.current_turn == Color::Black && game.black.as_ref() == Some(username) || game.current_turn == Color::White && game.white.as_ref() == Some(username)) {
                if let Some(sender) = server_state.user_connections.lock().await.get(username) {
                    sender.send(Message::Error("It's not your turn.".to_string())).await
                        .map_err(|e| ChessError::MessageHandlingError(format!("Failed to send message: {}", e)))?;
                }
                return Err(ChessError::GameStateError("It's not your turn.".to_string()));
            }

            game.make_move(&user_move)?;
            info!("Move made: {}", user_move);
            let game_is_finished: bool = game.result.is_some();

            let white_player = game.white.clone();
            let black_player = game.black.clone();

            drop(game);

            if game_is_finished {
                let mut finished_games = server_state.finished_games.lock().await;
                let mut games = server_state.games.lock().await;

                if let Some(game_arc) = games.remove(&game_id) {
                    finished_games.insert(game_id, game_arc);
                }

                let mut user_to_game = server_state.user_to_game.lock().await;
                if let Some(player) = white_player {
                    user_to_game.remove(&player);
                }
                if let Some(player) = black_player {
                    user_to_game.remove(&player);
                }
            }

            Ok(())
        } else {
            if let Some(sender) = server_state.user_connections.lock().await.get(username) {
                sender.send(Message::Error("You are not in a game. Start a game using /play.".to_string())).await
                    .map_err(|e| ChessError::MessageHandlingError(format!("Failed to send message: {}", e)))?;
            }
            Err(ChessError::GameStateError("Game not found".to_string()))
        }
    } else {
        Err(ChessError::GameStateError(format!("User {} is not currently in a game", username)))
    }
}

async fn start_game (game: &mut Game, server_state: &Arc<ServerState>) -> Result<(), ChessError> {
    let white_player = game.white.as_ref().ok_or(ChessError::GameStateError("White player missing".to_string()))?;
    let black_player = game.black.as_ref().ok_or(ChessError::GameStateError("Black player missing".to_string()))?;

    game.status = GameStatus::InProgress;

    info!("Starting a new game: {} as whites, {} as blacks.", white_player, black_player);
    send_game_state(game, server_state).await?;
    Ok(())
}

async fn assign_to_game(username: String, server_state: Arc<ServerState>) -> Result<(), ChessError> {
    //info!("Getting games");
    let mut games = server_state.games.lock().await;
    let mut user_game_assigned = false;
    let mut assigned_game_id = 0;

    //info!("Checking for an existing game with a player slot open");
    // Find an existing game with a player slot open
    for (&game_id, game_arc) in games.iter_mut() {
        let mut game = game_arc.lock().await;
        if game.white.is_none() {
            game.white = Some(username.clone());
            user_game_assigned = true;
            assigned_game_id = game_id;
            info!("{} is now white in game {}", username, game_id);
            break;
        } else if game.black.is_none() {
            game.black = Some(username.clone());
            user_game_assigned = true;
            assigned_game_id = game_id;
            info!("{} is now black in game {}", username, game_id);
            let _ = start_game(&mut game, &server_state).await?;
            break;
        }
    }

    info!("Creating a new game");
    // If no open games, create a new one
    if !user_game_assigned {
        let new_game_id = server_state.get_new_game_id();
        let new_game = Game {
            board: Board::default(),
            current_turn: chess::Color::White,
            white: Some(username.clone()),
            black: None,
            status: chess_game::GameStatus::Pending,
            result: None,
        };
        games.insert(new_game_id, Arc::new(Mutex::new(new_game)));
        assigned_game_id = new_game_id;
        info!("New game created for {} with game ID {}", username, new_game_id);
    }

    server_state.user_to_game.lock().await.insert(username.clone(), assigned_game_id);

    info!("Informing the user that they are in a game.");
    if let Some(sender) = server_state.user_connections.lock().await.get(&username) {
        send_message(&username, Message::Log(format!("You're in a game now!")), sender).await?;
        Ok(())
    } else {
        Err(ChessError::UserNotFoundError)
    }
}

async fn send_message(username: &str, message: Message, sender: &Sender<Message>) -> Result<(), ChessError> {
    info!("Trying to send message {:?} to {username}", message);

    if let Err(e) = sender.send(message).await {
        return Err(ChessError::MessageHandlingError(format!("Failed to send message to {}: {}", username, e)));
    } else {
        info!("Successfully sent message to {username}");
        Ok(())
    }
}