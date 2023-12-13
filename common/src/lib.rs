use serde::{Serialize, Deserialize};
use chess::{Board, ChessMove, Color, Piece, Square, MoveGen};
use std::io::{self, Write};
use std::str::FromStr;
use env_logger::Env;
use log::{info, error, debug};

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

pub fn piece_to_unicode(piece: Option<(Piece, Color)>) -> char {
    match piece {
        Some((Piece::Pawn, Color::White)) => '♙',
        Some((Piece::Knight, Color::White)) => '♘',
        Some((Piece::Bishop, Color::White)) => '♗',
        Some((Piece::Rook, Color::White)) => '♖',
        Some((Piece::Queen, Color::White)) => '♕',
        Some((Piece::King, Color::White)) => '♔',
        Some((Piece::Pawn, Color::Black)) => '♟',
        Some((Piece::Knight, Color::Black)) => '♞',
        Some((Piece::Bishop, Color::Black)) => '♝',
        Some((Piece::Rook, Color::Black)) => '♜',
        Some((Piece::Queen, Color::Black)) => '♛',
        Some((Piece::King, Color::Black)) => '♚',
        None => ' ',
    }
}

pub fn print_board(board: &Board) {
    println!("     A  B  C  D  E  F  G  H ");
    println!("   ┌──┬──┬──┬──┬──┬──┬──┬──┐");
    for rank in (1..=8).rev() {
        print!(" {} │", rank);
        for file in 'a'..='h' {
            let square = Square::from_str(&format!("{}{}", file, rank)).unwrap();
            let piece = board.piece_on(square);
            let color = board.color_on(square);
            print!("{} │", piece_to_unicode(piece.zip(color)));
        }
        if rank > 1 {
            println!("\n   ├──┼──┼──┼──┼──┼──┼──┼──┤"); 
        }
    }
    println!("\n   └──┴──┴──┴──┴──┴──┴──┴──┘");
}