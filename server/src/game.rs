use crate::User;

#[derive(Debug)]
pub struct Game {
    pub id: String,
    pub status: GameStatus,
    pub player1: Option<User>,
    pub player2: Option<User>,
}

#[derive(Debug)]
pub enum GameStatus {
    Waiting,
    PlacingShips,
    Started,
}
