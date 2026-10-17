use std::{
    collections::HashMap,
    error::Error,
    io,
    net::SocketAddr,
    sync::{Arc, LazyLock},
    time::Duration,
};

use axum::{
    extract::{
        connect_info::ConnectInfo,
        ws::{Message, WebSocket, WebSocketUpgrade, rejection::WebSocketUpgradeRejection},
    },
    response::{Html, IntoResponse, Response},
};
use futures_util::{FutureExt as _, StreamExt};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, info};

static STATE: LazyLock<Arc<Mutex<Shared>>> = LazyLock::new(|| Arc::new(Mutex::new(Shared::new())));

pub async fn ws_handler(ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>, ConnectInfo(addr): ConnectInfo<SocketAddr>) -> Response {
    match ws {
        Ok(ws) => ws.on_upgrade(move |socket| process(socket, addr).map(|_res| ())).into_response(),
        Err(_) => Html(include_str!("../assets/ws-chat.html")).into_response(),
    }
}

#[derive(Debug)]
struct Shared {
    peers: HashMap<Id, mpsc::UnboundedSender<Message>>,
}
type Id = uuid::Uuid;

#[derive(Debug)]
struct Peer {
    socket: WebSocket,
    rx: mpsc::UnboundedReceiver<Message>,
}

impl Shared {
    fn new() -> Self {
        Shared { peers: HashMap::new() }
    }

    async fn broadcast(&mut self, _sender: Id, message: &Message) {
        // debug!("{:?}", &self.peers);
        for (id, to_user) in self.peers.iter_mut() {
            println!("{} -> {:?}", id, message);
            // if *peer.0 != sender {
            let _ = to_user.send(message.clone());
            // }
        }
    }
}

impl Peer {
    async fn new(state: Arc<Mutex<Shared>>, socket: WebSocket, who: Id) -> io::Result<Peer> {
        let addr = who;
        let (tx, rx) = mpsc::unbounded_channel();
        state.lock().await.peers.insert(addr, tx);
        Ok(Peer { socket, rx })
    }
}

async fn process(ws: WebSocket, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
    let state = &STATE.clone();
    let mut lines = ws;
    lines.send(("Please enter your username:").into()).await?;

    let username = match lines.next().await {
        Some(Ok(Message::Text(line))) => line,
        _ => {
            tracing::error!("Failed to get username from {}. Client disconnected.", addr);
            return Ok(());
        }
    };
    let addr = uuid::Uuid::new_v4();
    let mut peer = Peer::new(state.clone(), lines, addr).await?;
    {
        let mut state = state.lock().await;
        let msg = format!("{} has joined the chat", username);
        info!("{}", msg);
        state.broadcast(addr, &msg.into()).await;
    }

    loop {
        let res = tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(10)) => Action::Timeout(()),
            Some( msg) = peer.rx.recv() => Action::RequireSend(msg),
            result = peer.socket.next() => Action::ReceiveMessage(result),
        };
        match res {
            Action::Timeout(()) => peer.socket.send(Message::Ping(vec![].into())).await?,
            Action::RequireSend(msg) => peer.socket.send(msg).await?,
            Action::ReceiveMessage(result) => match result {
                Some(Ok(msg)) => {
                    debug!("{:?}", msg);
                    match msg {
                        Message::Text(msg) => {
                            let mut state = state.lock().await;
                            let msg = format!("{username}({addr}): {msg}");
                            state.broadcast(addr, &msg.into()).await;
                        }
                        Message::Ping(_) => peer.socket.send(Message::Pong("".into())).await?,
                        Message::Binary(_) => (),
                        Message::Pong(_) => (),
                        Message::Close(_) => break,
                    }
                }
                Some(Err(e)) => {
                    tracing::error!("an error occurred while processing messages for {}; error = {:?}", username, e);
                }
                None => break,
            },
        }
    }
    {
        let mut state = state.lock().await;
        state.peers.remove(&addr);

        let msg = format!("{} has left the chat", username);
        tracing::info!("{}", msg);
        state.broadcast(addr, &msg.into()).await;
    }

    Ok(())
}

enum Action {
    Timeout(()),
    ReceiveMessage(Option<Result<Message, axum::Error>>),
    RequireSend(Message),
}
