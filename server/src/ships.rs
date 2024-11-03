use serde::{Deserialize, Serialize};

const SHIPS_LIMIT: usize = 10;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Ships {
    pub ships: Vec<Ship>,
}

impl Default for Ships {
    fn default() -> Self {
        Self {
            ships: Vec::with_capacity(SHIPS_LIMIT),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ShipType {
    Small,
    Medium,
    Large,
    Huge,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
// #[serde(default)]
pub struct Ship {
    pub position: Position,
    #[serde(rename = "direction")]
    pub is_vertical: bool,
    #[serde(rename = "type")]
    pub ship_type: ShipType,
    #[serde(rename = "length")]
    pub hp: u8,
}

// impl Default for Ship {
//     fn default() -> Self {
//         Self {
//             position: Position { x: 0, y: 0 },
//             is_vertical: false,
//             ship_type: ShipType::Small,
//             hp: 1,
//         }
//     }
// }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Position {
    pub x: u8,
    pub y: u8,
}
