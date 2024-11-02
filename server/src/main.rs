//! A chat server that broadcasts a message to all connections.
//!
//! This is a simple line-based server which accepts WebSocket connections,
//! reads lines from those connections, and broadcasts the lines to all other
//! connected clients.
//!
//! You can test this out by running:
//!
//!     cargo run --example server 127.0.0.1:12345
//!
//! And then in another window run:
//!
//!     cargo run --example client ws://127.0.0.1:12345/
//!
//! You can run the second command in multiple windows and then chat between the
//! two, seeing the messages from the other client as they're received. For all
//! connected clients they'll all join the same room and see everyone else's
//! messages.

use std::time::Duration;
use std::{
    collections::HashMap,
    env,
    io::Error as IoError,
    net::SocketAddr,
    sync::{Arc, Mutex, RwLock},
};
use tokio::{task, time}; // 1.3.0

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, pin_mut, stream::TryStreamExt, StreamExt};

use server::{ServerEvent, ServerState, State};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::protocol::Message;

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

async fn handle_connection(
    peer_map: PeerMap,
    raw_stream: TcpStream,
    addr: SocketAddr,
    mut state: ServerState,
) {
    println!("Incoming TCP connection from: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    println!("WebSocket connection established: {}", addr);

    // Insert the write part of this peer to the peer map.
    let (tx, rx) = unbounded();
    peer_map.lock().unwrap().insert(addr, tx);

    let (outgoing, incoming) = ws_stream.split();

    let broadcast_incoming = incoming.try_for_each(|msg| {
        println!(
            "Received a message from {}: {}",
            addr,
            msg.to_text().unwrap()
        );
        server::handle_event(&addr, msg.to_text().unwrap(), &mut state);
        // println!("{:?}", state);

        future::ok(())
    });

    let receive_from_others = rx.map(Ok).forward(outgoing);

    pin_mut!(broadcast_incoming, receive_from_others);
    future::select(broadcast_incoming, receive_from_others).await;

    println!("{} disconnected", &addr);
    server::handle_disconnect(&addr, &mut state);
    peer_map.lock().unwrap().remove(&addr);
}

async fn tick(peer_map: PeerMap, mut state: ServerState) {
    let mut interval = time::interval(Duration::from_millis(200));

    loop {
        interval.tick().await;
        server::tick(&mut state);

        let mut lock = state.write().unwrap();
        while let Some(event) = lock.events.dequeue() {
            let peers = peer_map.lock().unwrap();
            // We want to broadcast the message to everyone except ourselves.
            let broadcast_recipients = peers
                .iter()
                .filter(|(peer_addr, _)| match &event {
                    ServerEvent::All(_) => true,
                    ServerEvent::User(to, _) => peer_addr == &to,
                })
                .map(|(_, ws_sink)| ws_sink);

            let json = match &event {
                ServerEvent::All(json) | ServerEvent::User(_, json) => json,
            };

            for recp in broadcast_recipients {
                recp.unbounded_send(json.clone().into()).unwrap();
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), IoError> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:3000".to_string());

    let state = PeerMap::new(Mutex::new(HashMap::new()));

    let server_state: ServerState = Arc::new(RwLock::new(State::default()));

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    println!("Listening on: {}", addr);

    task::spawn(tick(state.clone(), server_state.clone()));

    // Let's spawn the handling of each connection in a separate task.
    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(
            state.clone(),
            stream,
            addr,
            server_state.clone(),
        ));
    }

    Ok(())
}
