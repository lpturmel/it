use it_core::{
    AcceptEvent, ClientEvent, ClientId, IntoResponse, LeaveEvent, LobbyId, Player, PosUpdateEvent,
    Position, ServerEvent, StartEvent, UdpUpgradeEvent,
};
use std::collections::HashMap;
use std::fmt::Display;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};

struct Server {
    lobbies: HashMap<LobbyId, Vec<Player>>,
    tcp_clients: HashMap<ClientId, mpsc::UnboundedSender<String>>,
    udp_tx: mpsc::UnboundedSender<(SocketAddr, Vec<u8>)>,
    udp_client_addrs: HashMap<ClientId, SocketAddr>,
}

impl Server {
    fn new(udp_tx: mpsc::UnboundedSender<(SocketAddr, Vec<u8>)>) -> Self {
        Self {
            lobbies: HashMap::new(),
            udp_tx,
            tcp_clients: HashMap::new(),
            udp_client_addrs: HashMap::new(),
        }
    }
}

struct Client {
    addr: SocketAddr,
    tcp: mpsc::UnboundedSender<String>,
}

const MAX_LOBBY_SIZE: usize = 2;

impl Server {
    fn remove_from_lobby(&mut self, client_id: &ClientId) -> Option<(LobbyId, ClientId)> {
        if let Some((lobby_id, clients)) = self
            .lobbies
            .iter_mut()
            .find(|(_, clients)| clients.iter().any(|p| p.id == *client_id))
        {
            clients.retain(|p| p.id != *client_id);
            info!("Client {} removed from lobby {}", client_id, lobby_id);

            return Some((lobby_id.clone(), client_id.clone()));
        }
        None
    }
    fn is_full(&self, lobby_id: &LobbyId) -> bool {
        self.lobbies.get(lobby_id).unwrap().len() >= MAX_LOBBY_SIZE
    }
    fn send(&self, client_id: &ClientId, event: impl IntoResponse) {
        if let Some(client_tx) = self.tcp_clients.get(client_id) {
            client_tx.send(event.into_response()).unwrap_or(());
        }
    }
    fn broadcast(&self, lobby_id: &LobbyId, event: impl IntoResponse) {
        let event = event.into_response();
        if let Some(clients) = self.lobbies.get(lobby_id) {
            for client in clients {
                if let Some(client_tx) = self.tcp_clients.get(&client.id) {
                    client_tx.send(event.clone()).unwrap_or(());
                }
            }
        }
    }
    fn broadcast_udp(&self, lobby_id: &LobbyId, client_id: &ClientId, msg: &str) {
        info!("Broadcasting to {}: {}", client_id, msg);
        if let Some(clients) = self.lobbies.get(lobby_id) {
            for client in clients {
                if client.id == *client_id {
                    continue;
                }
                if let Some(socket_addr) = self.udp_client_addrs.get(&client.id) {
                    self.udp_tx
                        .send((*socket_addr, msg.as_bytes().to_vec()))
                        .unwrap_or(());
                }
            }
        }
    }
    fn assign_to_lobby(&mut self, client_id: &ClientId) -> Result<LobbyId, Error> {
        for (lobby_id, lobby) in self.lobbies.iter_mut() {
            if lobby.len() < MAX_LOBBY_SIZE {
                let player = Player {
                    id: client_id.clone(),
                    it_count: 0,
                    position: Position { x: 0.0, y: 0.0 },
                };
                lobby.push(player);
                return Ok(lobby_id.clone());
            }
        }
        let new_lobby_id = uuid::Uuid::new_v4().to_string();
        let player = Player {
            id: client_id.clone(),
            it_count: 0,
            position: Position { x: 0.0, y: 0.0 },
        };
        self.lobbies.insert(new_lobby_id.clone(), vec![player]);

        Ok(new_lobby_id)
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let udp_socket = Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:8081").await.unwrap());
    let (udp_tx, mut udp_rx) = mpsc::unbounded_channel::<(SocketAddr, Vec<u8>)>();

    let server = Arc::new(RwLock::new(Server::new(udp_tx)));

    let udp_socket_clone = udp_socket.clone();
    tokio::spawn(async move {
        while let Some((addr, msg)) = udp_rx.recv().await {
            udp_socket_clone.send_to(&msg, addr).await.unwrap_or(0);
        }
    });

    let udp_server = server.clone();
    let udp_socket_clone = udp_socket.clone();
    tokio::spawn(async move {
        if let Err(e) = handle_udp(udp_socket_clone, udp_server).await {
            error!("Error handling UDP: {}", e);
        }
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    info!("Listening on http://127.0.0.1:8080");

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let state = server.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, state).await {
                error!("Error handling client: {}", e);
            }
        });
    }
}

#[derive(Debug)]
enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

async fn handle_client(
    stream: tokio::net::TcpStream,
    state: Arc<RwLock<Server>>,
) -> Result<(), Error> {
    let (reader, writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    let (tx, mut rx) = mpsc::unbounded_channel();

    let new_client_id = uuid::Uuid::new_v4().to_string();
    state
        .write()
        .await
        .tcp_clients
        .insert(new_client_id.clone(), tx.clone());

    info!("Client {} connected", new_client_id);

    let mut writer = writer;

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if writer.write_all(msg.as_bytes()).await.is_err() {
                break;
            }
        }
    });

    while reader.read_line(&mut line).await? != 0 {
        let cmd = line.trim();
        let event = serde_json::from_str::<ClientEvent>(cmd)?;

        match event {
            ClientEvent::Join => {
                info!("Received JOIN command");

                let mut state = state.write().await;

                let lobby_id = state.assign_to_lobby(&new_client_id)?;

                let accept_event = ServerEvent::Accept(AcceptEvent {
                    lobby_id: lobby_id.clone(),
                    client_id: new_client_id.clone(),
                });
                state.send(&new_client_id, accept_event);

                if state.is_full(&lobby_id) {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let players = state.lobbies.get(&lobby_id).unwrap().clone();

                    for player in &players {
                        let event = ServerEvent::Start(StartEvent {
                            lobby_id: lobby_id.clone(),
                            client_id: player.id.clone(),
                            players: players.to_vec(),
                        });
                        state.send(&player.id, event);
                    }
                } else {
                    let event = ServerEvent::Wait;
                    let event = event.into_response();
                    tx.send(event).unwrap_or(());
                }
            }
            _ => {
                error!("Unknown command: {}", cmd);
            }
        }

        line.clear();
    }
    // Cleanup
    {
        let mut state = state.write().await;
        state.tcp_clients.remove(&new_client_id);
        let removed = state.remove_from_lobby(&new_client_id);
        if let Some((lobby_id, client_id)) = removed {
            let lobby = state.lobbies.get(&lobby_id).unwrap();

            state.broadcast(&lobby_id, ServerEvent::Leave(LeaveEvent { client_id }));

            if lobby.is_empty() {
                state.lobbies.remove(&lobby_id);
                info!("Lobby {} removed as it's empty", lobby_id);
            }
        }
    }
    info!("Client {} disconnected", new_client_id);

    Ok(())
}

async fn handle_udp(
    socket: Arc<tokio::net::UdpSocket>,
    state: Arc<RwLock<Server>>,
) -> Result<(), Error> {
    let mut buf = [0u8; 1024];

    loop {
        let (len, addr) = socket.recv_from(&mut buf).await?;
        let msg = String::from_utf8_lossy(&buf[..len]);

        let event = serde_json::from_str::<ClientEvent>(&msg)?;

        match event {
            ClientEvent::UdpUpgrade(UdpUpgradeEvent { client_id }) => {
                let mut state = state.write().await;
                state.udp_client_addrs.insert(client_id, addr);
            }
            ClientEvent::PosUpdate(PosUpdateEvent { client_id, x, y }) => {
                let state = state.read().await;
                let lobby = state
                    .lobbies
                    .values()
                    .find(|lobby| lobby.iter().any(|p| p.id == *client_id));
                if let Some(players) = lobby {
                    for player in players {
                        if player.id != *client_id {
                            let event = ServerEvent::PosUpdate(PosUpdateEvent {
                                client_id: client_id.clone(),
                                x,
                                y,
                            });
                            let player_addr = state.udp_client_addrs.get(&player.id).unwrap();
                            state
                                .udp_tx
                                .send((*player_addr, event.into_response().as_bytes().to_vec()))
                                .unwrap_or(());
                        }
                    }
                }
            }
            ClientEvent::Join => {}
        }
    }
}
