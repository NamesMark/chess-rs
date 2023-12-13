use chess::{Board, Color, Piece, Square};

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