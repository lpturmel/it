use serde::{Deserialize, Serialize};

pub type LobbyId = String;
pub type ClientId = String;

pub trait IntoResponse {
    fn into_response(self) -> String;
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Player {
    pub id: ClientId,
    /// Number of times the player has been 'it'
    pub it_count: usize,
    pub position: Position,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientEvent {
    Join,
    UdpUpgrade(UdpUpgradeEvent),
    PosUpdate(PosUpdateEvent),
}
impl IntoResponse for ClientEvent {
    fn into_response(self) -> String {
        let mut str = serde_json::to_string(&self).unwrap();
        str.push('\n');
        str
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UdpUpgradeEvent {
    pub client_id: ClientId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PosUpdateEvent {
    pub client_id: ClientId,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ServerEvent {
    Start(StartEvent),
    Wait,
    Accept(AcceptEvent),
    Leave(LeaveEvent),
    PosUpdate(PosUpdateEvent),
}

impl IntoResponse for ServerEvent {
    fn into_response(self) -> String {
        let mut str = serde_json::to_string(&self).unwrap();
        str.push('\n');
        str
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AcceptEvent {
    pub lobby_id: LobbyId,
    pub client_id: ClientId,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StartEvent {
    pub lobby_id: LobbyId,
    pub client_id: ClientId,
    pub players: Vec<Player>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LeaveEvent {
    pub client_id: ClientId,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientInitEvent {
    pub client_id: ClientId,
}
