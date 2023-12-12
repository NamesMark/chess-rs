
use shakmaty::{Chess, Position};
use shakmaty::{Square, Move, Role};
use san_rs::*;

let bool curr_player; // 0 white, 1 black

fn process_move (player: bool, pl_move_str: String) {
    if (player != player) {
        // Err not your move yet
    }
    move_data = parse_move(pl_move_str);
    
    // check if move is valid

    // update the board

    // return Ok(())
}

fn parse_move (pl_move_str: String) {
    let san_move_data = Move::parse(pl_move_str).unwrap();
    let sh_move_data: shakmaty::Move = // convert move data from san_rs to shakmaty
    
}