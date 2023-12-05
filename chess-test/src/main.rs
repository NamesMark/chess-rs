use chess::{Board, ChessMove, Color, Piece, Square, MoveGen};
use std::io::{self, Write};
use std::str::FromStr;
use env_logger::Env;
use log::{info, error, debug};

fn piece_to_unicode(piece: Option<(Piece, Color)>) -> char {
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

fn print_board(board: &Board) {
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

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let mut board = Board::default();
    let mut turn = Color::White;

    loop {
        print_board(&board);
        debug!("{}",&board);

        let legal_moves = MoveGen::new_legal(&board);

        // Check for check
        if board.checkers().popcnt() > 0 {
            info!("Check!");
            if legal_moves.count() == 0 {
                info!("{:?} wins by checkmate!", !turn);
                break;
            }
        }

        // Prompt for user input
        info!("Enter move for {:?} (or type 'concede' to give up): ", turn);
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        // Concede command
        if input.trim().eq_ignore_ascii_case("concede") {
            info!("{:?} concedes. {:?} wins!", turn, !turn);
            break;
        }

        // Parse and apply the move
        match ChessMove::from_str(&input.trim()) {
            Ok(mov) => {
                if board.legal(mov) {
                    board = board.make_move_new(mov);
                    turn = !turn;
                    info!("Move made: {:?}", mov);
                    debug!("Board after move:\n{}", board);
                } else {
                    error!("Invalid move. Try again.");
                }
            }
            Err(_) => {
                error!("Couldn't parse input. Please use algebraic notation (e.g., e2e4).");
            }
        }
    }
}