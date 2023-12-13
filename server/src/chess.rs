use chess::{Board, ChessMove, Color, Piece, Square, MoveGen};
use std::io::{self, Write};
use std::str::FromStr;
use env_logger::Env;
use log::{info, error, debug};

pub fn start_game() {
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
                error!("Couldn't parse input. Please use long algebraic notation (e.g., e2e4).");
            }
        }
    }
}