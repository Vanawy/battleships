use crate::{ships::Ships, User, UserId};

const BOARD_SIZE: usize = 10;

pub type GameId = String;

#[derive(Debug)]
pub struct Game {
    pub id: GameId,
    pub status: GameStatus,
    pub player1: Option<UserId>,
    pub player2: Option<UserId>,

    is_p1_turn: bool,
    p1_board: Board,
    p2_board: Board,
}

impl Game {
    pub fn create(id: &GameId, p1: &User) -> Self {
        Self {
            id: id.to_string(),
            status: GameStatus::Waiting,
            player1: Some(p1.id.clone()),
            player2: None,
            is_p1_turn: rand::random::<bool>(),

            p1_board: Board::default(),
            p2_board: Board::default(),
        }
    }

    pub fn add_ships(&mut self, ships: &Ships, user_id: &UserId) {
        let board: &mut Board = if user_id.clone() == self.player1.clone().unwrap() {
            &mut self.p1_board
        } else {
            &mut self.p2_board
        };

        let mut i = 0;

        for ship in ships.ships.clone() {
            let mut pos = ship.position.clone();
            for _ in 0..ship.hp {
                board.set_cell(pos.x, pos.y, Cell::Alive(i));
                if ship.is_vertical {
                    pos.y += 1;
                } else {
                    pos.x += 1;
                }
            }
            i += 1;
            board.ships.ships.push(ship);
        }
        println!("{}", board.to_string());
    }
}

#[derive(Debug)]
struct Board {
    ships: Ships,
    cells: Vec<Cell>,
}

impl Board {
    fn set_cell(&mut self, x: u8, y: u8, cell: Cell) {
        self.cells[x as usize + y as usize * BOARD_SIZE] = cell;
    }
}

impl Default for Board {
    fn default() -> Self {
        Self {
            ships: Ships::default(),
            cells: Vec::from([Cell::Empty; BOARD_SIZE * BOARD_SIZE]),
        }
    }
}

impl ToString for Board {
    fn to_string(&self) -> String {
        let mut s = String::new();
        for y in (0..BOARD_SIZE) {
            for x in 0..BOARD_SIZE {
                match self.cells[x as usize + y as usize * BOARD_SIZE] {
                    Cell::Empty => s.push('-'),
                    Cell::Alive(v) => s.push_str(&v.to_string()),
                    Cell::Miss => s.push('x'),
                    Cell::Shot => s.push('+'),
                    Cell::Killed => s.push('D'),
                };
            }
            s.push('\n');
        }
        s
    }
}

#[derive(Debug, Clone, Copy)]
enum Cell {
    Empty,
    Alive(usize),
    Miss,
    Shot,
    Killed,
}

#[derive(Debug)]
pub enum GameStatus {
    Waiting,
    PlacingShips,
    Started,
}
