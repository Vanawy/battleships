use queue::Queue;
use serde_json::json;
use std::{
    collections::HashMap,
    net::SocketAddr,
    ops::Deref,
    str::FromStr,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

mod game;

use game::{Game, GameStatus};

use serde::{Deserialize, Serialize};

#[derive(Debug)]
enum ClientEvent {
    Player(PlayerEvent),
    Room(RoomEvent),
    Ships(ShipsEvent),
    Game(GameEvent),
}

#[derive(Debug)]
enum PlayerEvent {
    Reg(Registration),
}

#[derive(Debug)]
struct Registration {
    username: String,
}

#[derive(Debug)]
enum RoomEvent {
    Create,
    AddUser,
    Update,
}

#[derive(Debug)]
enum ShipsEvent {
    Add,
    Start,
}

#[derive(Debug)]
enum GameEvent {
    Attack,
    RandomAttack,
    Turn,
}

struct Error {
    text: String,
}

pub type ServerState = Arc<RwLock<State>>;

#[derive(Debug)]
pub struct State {
    pub events: Queue<ServerEvent>,
    users: HashMap<SocketAddr, User>,
    games: HashMap<String, Game>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            events: Queue::new(),
            users: HashMap::new(),
            games: HashMap::new(),
        }
    }
}

impl State {
    fn add_user(&mut self, user: &User) -> User {
        let user = self.users.entry(user.addr).or_insert(user.clone()).clone();
        self.add_update_winners_event();
        self.add_update_room_event();
        user
    }
    fn update_user(&mut self, user: &User) {
        self.users.insert(user.addr, user.clone());
    }
    fn add_event(&mut self, event: &ServerEvent) {
        let _ = self.events.queue(event.clone());
    }

    fn add_update_winners_event(&mut self) {
        let json = serde_json::Value::Array(
            self.users
                .values()
                .into_iter()
                .map(|user| {
                    json!({
                        "name": user.name,
                        "wins": user.wins,
                    })
                })
                .collect::<Vec<serde_json::Value>>(),
        );
        self.add_event(&ServerEvent::All(create_event_json(
            json,
            "update_winners".into(),
        )));
    }

    fn add_update_room_event(&mut self) {
        let json = serde_json::Value::Array(
            self.games
                .values()
                .into_iter()
                .filter_map(|game| match game.status {
                    GameStatus::Waiting => Some(json!({
                        "roomId": game.id,
                        "roomUsers": serde_json::Value::Array([&game.player1, &game.player2]
                            .into_iter()
                            .filter_map(|user| {
                                match user {
                                    Some(user) => {
                                        Some(json!({
                                            "name": user.name,
                                            "index": user.id,
                                        }))
                                    }
                                    None => None
                                }
                            })
                            .collect::<Vec<serde_json::Value>>(),
                        ),
                    })),
                    _ => None,
                })
                .collect::<Vec<serde_json::Value>>(),
        );
        self.add_event(&ServerEvent::All(create_event_json(
            json,
            "update_room".into(),
        )));
    }

    fn create_game(&mut self, user: &User) {
        if let Some(room) = user.in_room.clone() {
            if self.games.contains_key(&room) {
                println!("USER IN ROOM");
                return;
            }
        }

        let game_id = Uuid::new_v4();

        self.games.insert(
            game_id.to_string(),
            Game {
                id: game_id.to_string(),
                status: GameStatus::Waiting,
                player1: Some(user.clone()),
                player2: None,
            },
        );

        let user = User {
            in_room: Some(game_id.to_string()),
            ..user.clone()
        };

        self.update_user(&user);

        let json = json!({
            "idGame": game_id.to_string(),
            "idPlayer": user.id,
        });
        self.add_update_room_event();
    }

    fn add_user_to_game() {}
}

#[derive(Debug, Clone)]
struct User {
    id: String,
    name: String,
    addr: SocketAddr,
    wins: u32,
    in_room: Option<String>,
}

#[derive(Serialize)]
struct ServerEventData {
    #[serde(rename = "type")]
    type_str: String,
    data: String,
    id: u32,
}

#[derive(Debug, Clone)]
pub enum ServerEvent {
    User(SocketAddr, String),
    // Game(String),
    All(String),
}

pub fn tick(state: &mut ServerState) {}

pub fn handle_event(addr: &SocketAddr, event_json: &str, state: &mut ServerState) {
    let json: serde_json::Value =
        serde_json::from_str(event_json).unwrap_or(serde_json::Value::Null);
    println!("Received json: {:?}", json);
    let event = parse_event(&addr, json);

    match event {
        Ok(event) => {
            println!("Event {:?}", event);
            let user = {
                let lock = state.read().unwrap();
                lock.users.get(&addr).cloned()
            };

            match event {
                ClientEvent::Player(player_event) => match player_event {
                    PlayerEvent::Reg(reg) => match user {
                        Some(user) => {}
                        None => {
                            let uuid = Uuid::new_v4();
                            let user = User {
                                id: uuid.to_string(),
                                name: reg.username.clone(),
                                addr: addr.clone(),
                                wins: 0,
                                in_room: None,
                            };

                            let mut lock = state.write().unwrap();
                            let user = lock.add_user(&user);
                            lock.add_update_room_event();

                            let data = json!({
                                "name": user.name,
                                "index": user.id,
                                "error": false,
                                "errorText": "",
                            });
                            let json = create_event_json(data, "reg".into());

                            lock.add_event(&ServerEvent::User(user.addr, json));
                        }
                    },
                },
                ClientEvent::Room(room_event) => match room_event {
                    RoomEvent::Create => {
                        let user = user.unwrap();
                        state.write().unwrap().create_game(&user);
                    }
                    _ => {}
                },

                _ => {}
            }
        }
        Err(err) => {}
    };
}

pub fn handle_disconnect(addr: &SocketAddr, state: &mut ServerState) {
    if let Some(user) = state.write().unwrap().users.remove(&addr) {
        println!("User '{}' disconnected ({})", user.name, user.addr);
    }
}
// fn create_room(User)

fn create_event_json(data: serde_json::Value, event_type: String) -> String {
    let data = serde_json::to_string(&data).unwrap();

    let json = serde_json::to_string(&ServerEventData {
        type_str: event_type,
        data: data,
        id: 0,
    })
    .unwrap();

    json
}

fn parse_event(addr: &SocketAddr, json: serde_json::Value) -> Result<ClientEvent, Error> {
    println!("json <- {:?}", json);

    let event_type: &str = json["type"].as_str().unwrap_or("unknown");

    let data_json: serde_json::Value = serde_json::from_str(json["data"].as_str().unwrap_or(""))
        .unwrap_or(serde_json::Value::Null);
    println!("Event Type - {:?}", event_type);
    println!("Data: {:?}", data_json);

    match event_type {
        "reg" => Ok(ClientEvent::Player(PlayerEvent::Reg(Registration {
            username: data_json["name"].as_str().unwrap().to_owned(),
        }))),
        "create_room" => Ok(ClientEvent::Room(RoomEvent::Create)),
        &_ => Err(Error {
            text: "Unknown event type".to_owned(),
        }),
    }
}
