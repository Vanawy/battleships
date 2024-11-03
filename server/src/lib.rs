use queue::Queue;
use serde_json::json;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

mod game;
mod ships;

use game::{Game, GameId, GameStatus};
use ships::Ships;

use serde::Serialize;

#[derive(Debug)]
enum ClientEvent {
    Player(PlayerEvent),
    Room(RoomEvent),
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
    AddUser(String),
}

#[derive(Debug)]
enum GameEvent {
    AddShips(Ships),
    Start,
    Attack,
    RandomAttack,
    Turn,
}
struct Error {
    text: String,
}

pub type ServerState = Arc<RwLock<State>>;

type UserId = String;

#[derive(Debug)]
pub struct State {
    pub events: Queue<ServerEvent>,
    user_ids: HashMap<SocketAddr, UserId>,
    users: HashMap<UserId, User>,
    games: HashMap<GameId, Game>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            events: Queue::new(),
            user_ids: HashMap::new(),
            users: HashMap::new(),
            games: HashMap::new(),
        }
    }
}

impl State {
    fn add_user(&mut self, user: &User) -> User {
        let user_id = self.user_ids.entry(user.addr).or_insert(user.id.clone());
        let user = self
            .users
            .entry(user_id.to_string())
            .or_insert(user.clone())
            .clone();
        self.add_update_winners_event();
        self.add_update_room_event();
        user
    }
    fn get_user_by_addr(&self, addr: &SocketAddr) -> Option<&User> {
        match self.user_ids.get(addr) {
            Some(user_id) => self.get_user(user_id),
            None => None,
        }
    }
    fn remove_user_by_addr(&mut self, addr: &SocketAddr) -> Option<User> {
        match self.user_ids.remove(addr) {
            Some(user_id) => self.users.remove(&user_id),
            None => None,
        }
    }
    fn get_user(&self, user_id: &String) -> Option<&User> {
        self.users.get(user_id)
    }
    fn update_user(&mut self, user: &User) {
        self.users.insert(user.id.clone(), user.clone());
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
                            .filter_map(|user_id| {
                                match user_id {
                                    Some(user_id) => match self.get_user(&user_id) {
                                        Some(user) => Some(json!({
                                            "name": user.name,
                                            "index": user_id.clone(),
                                        })),
                                        None => None
                                    },
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
        let game_id = Uuid::new_v4();
        self.join_game(game_id.to_string(), user, true);
    }

    fn join_game(&mut self, game_id: String, user: &User, is_owner: bool) {
        if let Some(room) = user.in_room.clone() {
            if self.games.contains_key(&room) {
                println!("USER IN ROOM");
                return;
            }
        }

        if is_owner {
            self.games
                .insert(game_id.clone(), Game::create(&game_id, user));
        } else {
            {
                let game = self.games.get_mut(&game_id).unwrap();
                game.player2 = Some(user.id.clone());
                game.status = GameStatus::PlacingShips;
            }

            if let Some(game) = self.games.get(&game_id) {
                let json = json!([{
                    "idGame": game.id.clone(),
                    "idPlayer": game.player1.clone(),
                }, {
                    "idGame": game.id.clone(),
                    "idPlayer": game.player2.clone(),
                }]);

                self.add_event(&ServerEvent::All(create_event_json(
                    json,
                    "create_game".into(),
                )));
            }
        }
        let user = User {
            in_room: Some(game_id.to_string()),
            ..user.clone()
        };
        self.update_user(&user);
        self.add_update_room_event();
    }

    fn add_ships_to_game(&mut self, user: &User, ships: Ships) {
        let game = self.games.get_mut(&user.in_room.clone().unwrap()).unwrap();
        game.add_ships(&ships, &user.id);
    }
}

#[derive(Debug, Clone)]
struct User {
    id: UserId,
    name: String,
    addr: SocketAddr,
    wins: u32,
    in_room: Option<GameId>,
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

pub fn tick(_state: &mut ServerState) {}

pub fn handle_event(addr: &SocketAddr, event_json: &str, state: &mut ServerState) {
    let json: serde_json::Value =
        serde_json::from_str(event_json).unwrap_or(serde_json::Value::Null);
    println!("Received json: {:?}", json);
    let event = parse_event(json);

    match event {
        Ok(event) => {
            println!("Event {:?}", event);
            let user = {
                let state_lock = state.read().unwrap();
                state_lock.get_user_by_addr(addr).cloned()
            };

            match event {
                ClientEvent::Player(player_event) => match player_event {
                    PlayerEvent::Reg(reg) => match user {
                        Some(_user) => {}
                        None => {
                            let uuid = Uuid::new_v4();
                            let user = User {
                                id: uuid.to_string(),
                                name: reg.username.clone(),
                                addr: addr.clone(),
                                wins: 0,
                                in_room: None,
                            };

                            let mut state_lock = state.write().unwrap();
                            let user = state_lock.add_user(&user);
                            state_lock.add_update_room_event();

                            let data = json!({
                                "name": user.name,
                                "index": user.id,
                                "error": false,
                                "errorText": "",
                            });
                            let json = create_event_json(data, "reg".into());

                            state_lock.add_event(&ServerEvent::User(user.addr, json));
                        }
                    },
                },
                ClientEvent::Room(room_event) => match room_event {
                    RoomEvent::Create => {
                        let user = user.unwrap();
                        state.write().unwrap().create_game(&user);
                    }
                    RoomEvent::AddUser(game_id) => {
                        let user = user.unwrap();
                        state.write().unwrap().join_game(game_id, &user, false);
                    }
                },
                ClientEvent::Game(game_event) => match game_event {
                    GameEvent::AddShips(ships) => {
                        let user = user.unwrap();
                        state.write().unwrap().add_ships_to_game(&user, ships);
                    }
                    _ => {}
                },
            }
        }
        Err(err) => {
            eprintln!("{}", err.text)
        }
    };
}

pub fn handle_disconnect(addr: &SocketAddr, state: &mut ServerState) {
    let mut state_lock = state.write().unwrap();
    if let Some(user) = state_lock.remove_user_by_addr(&addr) {
        if let Some(room_id) = user.in_room {
            state_lock.games.remove(&room_id);
            state_lock.add_update_room_event();
            println!("Room '{}' closed - owner left", room_id.to_string());
        }
        println!("User '{}' disconnected ({})", user.name, user.addr);
        state_lock.add_update_winners_event();
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

fn parse_event(json: serde_json::Value) -> Result<ClientEvent, Error> {
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
        "add_user_to_room" => Ok(ClientEvent::Room(RoomEvent::AddUser(
            data_json["indexRoom"].as_str().unwrap().to_owned(),
        ))),
        "add_ships" => {
            let ships: Ships = serde_json::from_value(data_json).unwrap();
            Ok(ClientEvent::Game(GameEvent::AddShips(ships.clone())))
        }
        &_ => Err(Error {
            text: "Unknown event type".to_owned(),
        }),
    }
}
