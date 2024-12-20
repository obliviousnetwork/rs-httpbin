use std::sync::{atomic::AtomicUsize, Arc};

use serde::{Deserialize, Serialize};
use socketioxide::{
    extract::{Data, Extension, SocketRef, State},
    layer::SocketIoLayer,
    SocketIo,
};

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(transparent)]
struct Username(String);

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase", untagged)]
enum Res {
    Login {
        #[serde(rename = "numUsers")]
        num_users: usize,
    },
    UserEvent {
        #[serde(rename = "numUsers")]
        num_users: usize,
        username: Username,
    },
    Message {
        username: Username,
        message: String,
    },
    Username {
        username: Username,
    },
}
#[derive(Clone)]
struct UserCnt(Arc<AtomicUsize>);
impl UserCnt {
    fn new() -> Self {
        Self(Arc::new(AtomicUsize::new(0)))
    }
    fn add_user(&self) -> usize {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
    }
    fn remove_user(&self) -> usize {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) - 1
    }
}
pub(crate) fn socket_io_layer() -> SocketIoLayer {
    let (socketio_layer, io) = SocketIo::builder().with_state(UserCnt::new()).build_layer();
    {
        // io.ns("/", socket_io_echo::on_connect);
        io.ns("/socketio-chat", on_connect);
    }
    socketio_layer
}

fn on_connect(s: SocketRef) {
    s.on(
        "new message",
        |s: SocketRef, Data::<String>(msg), Extension::<Username>(username)| {
            let msg = &Res::Message { username, message: msg };
            s.broadcast().emit("new message", msg).ok();
        },
    );

    s.on("add user", |s: SocketRef, Data::<String>(username), user_cnt: State<UserCnt>| {
        if s.extensions.get::<Username>().is_some() {
            return;
        }
        let num_users = user_cnt.add_user();
        s.extensions.insert(Username(username.clone()));
        s.emit("login", &Res::Login { num_users }).ok();

        let res = &Res::UserEvent {
            num_users,
            username: Username(username),
        };
        s.broadcast().emit("user joined", res).ok();
    });

    s.on("typing", |s: SocketRef, Extension::<Username>(username)| {
        s.broadcast().emit("typing", &Res::Username { username }).ok();
    });

    s.on("stop typing", |s: SocketRef, Extension::<Username>(username)| {
        s.broadcast().emit("stop typing", &Res::Username { username }).ok();
    });

    s.on_disconnect(|s: SocketRef, user_cnt: State<UserCnt>, Extension::<Username>(username)| {
        let num_users = user_cnt.remove_user();
        let res = &Res::UserEvent { num_users, username };
        s.broadcast().emit("user left", res).ok();
    });
}
