use std::str::FromStr;

use chess::{GameResult, Board, ChessMove, Color, MoveGen};
use log::info;

use common::ChessError;

#[derive(Debug)]
pub struct Game {
    pub board: Board,
    pub current_turn: Color,
    pub white: Option<String>,
    pub black: Option<String>,
    pub status: GameStatus,
    pub result: Option<GameResult>,
}

#[derive(Debug)]
pub enum GameStatus {
    Pending,
    InProgress,
    Finished,
    Cancelled
}

impl Game {
    pub fn new() -> Self {
        Self {
            board: Board::default(),
            current_turn: Color::White,
            white: None,
            black: None,
            status: GameStatus::Pending,
            result: None,
        }
    }

    pub fn make_move(&mut self, move_str: &str) -> Result<(), ChessError> {
        match ChessMove::from_str(move_str) {
            Ok(mov) => {
                if self.board.legal(mov) {
                    self.board = self.board.make_move_new(mov);
                    self.current_turn = !self.current_turn;
                    Ok(())
                } else {
                    Err(ChessError::GameStateError("Invalid move.".to_string()))
                }
            }
            Err(_) => Err(ChessError::GameStateError("Couldn't parse move.".to_string())),
        }
    }

    pub fn concede(&mut self, player: &String) -> Result<(), ChessError> {
        info!("{:?} concedes. {:?} wins!", self.current_turn, !self.current_turn);
        if self.white.as_ref() == Some(player) {
            self.result = Some(GameResult::WhiteResigns);
        } else if self.black.as_ref() == Some(player) {
            self.result = Some(GameResult::BlackResigns);
        } else {
            return Err(ChessError::UserNotFoundError);
        }
        self.status = GameStatus::Finished;
        Ok(())
    }

    pub fn is_check(&mut self) -> bool {
        if self.board.checkers().popcnt() > 0 {
            true
        } else {
            false
        }
    }

    pub fn is_mate(&mut self) -> bool {
        let legal_moves = MoveGen::new_legal(&self.board);

        if legal_moves.count() == 0 {
            info!("{:?} wins by checkmate!", !self.current_turn);
            if self.current_turn == Color::White {
                self.result = Some(GameResult::BlackCheckmates);
            } else {
                self.result = Some(GameResult::WhiteCheckmates);
            }
            self.status = GameStatus::Finished;
            true
        } else {
            false
        }
    }

    pub fn is_drawable(&mut self) -> bool {
        todo!() //if self.board.
    }

    pub fn check_result(&self) -> Option<GameResult> {
        // TODO draw
        // Some(GameResult::DrawAccepted)
        // Some(GameResult::WhiteResigns)
        // Some(GameResult::BlackResigns)
        unimplemented!()
    }

    // TODO en passant
}