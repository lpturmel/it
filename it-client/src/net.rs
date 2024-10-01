use crate::menu::MenuState;
use crate::player::{Player, SpawnPlayerEvent};
use crate::GameState;
use async_channel::{unbounded, Receiver, Sender};
use async_net::{TcpStream, UdpSocket};
use bevy::asset::AsyncWriteExt;
use bevy::prelude::*;
use bevy::tasks::futures_lite::io::BufReader;
use bevy::tasks::futures_lite::AsyncBufReadExt;
use bevy::tasks::IoTaskPool;
use it_core::{ClientEvent, ServerEvent, UdpUpgradeEvent};
use std::sync::Arc;
use tracing::info;

#[derive(Resource)]
pub struct TcpSocketSender(pub Sender<ClientEvent>);

#[derive(Resource)]
pub struct TcpSocketReceiver(pub Receiver<ServerEvent>);

#[derive(Resource)]
pub struct UdpSocketSender(pub Sender<ClientEvent>);

#[derive(Resource)]
pub struct UdpSocketReceiver(pub Receiver<ServerEvent>);

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_tcp)
            .add_systems(Update, on_tcp_event)
            .add_systems(Update, on_udp_event.run_if(in_state(GameState::Game)));
    }
}

fn on_udp_event(
    socket_receiver: ResMut<UdpSocketReceiver>,
    mut players_query: Query<(&mut Transform, &Player)>,
) {
    while let Ok(event) = socket_receiver.0.try_recv() {
        match event {
            ServerEvent::PosUpdate(pos_event) => {
                for (mut transform, player) in players_query.iter_mut() {
                    if player.id == pos_event.client_id {
                        transform.translation.x = pos_event.x;
                        transform.translation.y = pos_event.y;
                    }
                }
            }
            ServerEvent::Accept(_) => {}
            ServerEvent::Start(_) => {}
            ServerEvent::Wait => {}
            ServerEvent::Leave(_) => {}
        }
    }
}
fn on_tcp_event(
    socket_receiver: ResMut<TcpSocketReceiver>,
    udp_sender: ResMut<UdpSocketSender>,
    mut game_state: ResMut<NextState<GameState>>,
    mut menu_state: ResMut<NextState<MenuState>>,
    mut commands: Commands,
) {
    let task_pool = IoTaskPool::get();
    let udp_sender = udp_sender.0.clone();
    while let Ok(event) = socket_receiver.0.try_recv() {
        match event {
            ServerEvent::PosUpdate(_) => {
                // Do nothing in TCP
            }
            ServerEvent::Start(start_event) => {
                game_state.set(GameState::Game);
                menu_state.set(MenuState::Disabled);

                let player_client_id = start_event.client_id.clone();

                for player in start_event.players {
                    let is_main = player.id == player_client_id;
                    let player = SpawnPlayerEvent {
                        coords: Vec2::new(player.position.x, player.position.y),
                        id: player.id.clone(),
                        main_player: is_main,
                    };
                    commands.trigger(player);
                }
            }
            ServerEvent::Wait => {
                info!("Waiting for more players...");
            }
            ServerEvent::Accept(accept_event) => {
                let player_client_id = accept_event.client_id.clone();
                let udp_sender = udp_sender.clone();
                task_pool
                    .spawn(async move {
                        let _ = udp_sender
                            .send(ClientEvent::UdpUpgrade(UdpUpgradeEvent {
                                client_id: player_client_id,
                            }))
                            .await;
                    })
                    .detach();

                menu_state.set(MenuState::Lobby);
            }
            ServerEvent::Leave(leave_event) => {
                info!("Player {} left the game", leave_event.client_id);
            }
        }
    }
}

pub fn setup_tcp(mut commands: Commands) {
    let (tcp_client_sender, tcp_client_receiver) = unbounded::<ClientEvent>();
    let (tcp_server_sender, tcp_server_receiver) = unbounded::<ServerEvent>();

    let (udp_client_sender, udp_client_receiver) = unbounded::<ClientEvent>();
    let (udp_server_sender, udp_server_receiver) = unbounded::<ServerEvent>();

    commands.insert_resource(TcpSocketSender(tcp_client_sender));
    commands.insert_resource(TcpSocketReceiver(tcp_server_receiver));

    commands.insert_resource(UdpSocketSender(udp_client_sender));
    commands.insert_resource(UdpSocketReceiver(udp_server_receiver));

    let task_pool = IoTaskPool::get();
    task_pool
        .spawn(async move {
            if let Err(e) = tcp_socket_task(tcp_client_receiver, tcp_server_sender, task_pool).await
            {
                error!("Socket task error: {:?}", e);
            }
        })
        .detach();

    task_pool
        .spawn(async move {
            if let Err(e) = udp_socket_task(udp_client_receiver, udp_server_sender, task_pool).await
            {
                error!("UDP socket task error: {:?}", e);
            }
        })
        .detach();
}

async fn tcp_socket_task(
    client_receiver: Receiver<ClientEvent>,
    server_sender: Sender<ServerEvent>,
    task_pool: &IoTaskPool,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream = TcpStream::connect("127.0.0.1:8080").await?;
    info!("Connected to server");

    let reader = stream.clone();
    let writer = stream;

    let reader = BufReader::new(reader);

    let mut line = String::new();

    let server_sender_clone = server_sender.clone();
    let read_task = async move {
        let mut reader = reader;
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    info!("Server closed the connection");
                    break;
                }
                Ok(_) => {
                    let response = line.trim();
                    match serde_json::from_str::<ServerEvent>(response) {
                        Ok(event) => {
                            let _ = server_sender_clone.send(event).await;
                        }
                        Err(e) => {
                            error!("Failed to parse server event: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to read from server: {:?}", e);
                    break;
                }
            }
        }
    };

    let write_task = async move {
        let mut writer = writer;
        while let Ok(event) = client_receiver.recv().await {
            info!("Receiver sending event over TCP...");
            let msg = serde_json::to_string(&event)? + "\n";
            writer.write_all(msg.as_bytes()).await?;
            writer.flush().await?;
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    };

    task_pool.spawn(read_task).detach();
    write_task.await?;

    Ok(())
}

async fn udp_socket_task(
    client_receiver: Receiver<ClientEvent>,
    server_sender: Sender<ServerEvent>,
    task_pool: &IoTaskPool,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
    info!("UDP socket bound to {}", socket.local_addr()?);

    socket.connect("127.0.0.1:8081").await?;
    info!("Connected to UDP server at 127.0.0.1:8081");

    let socket_clone = socket.clone();

    let server_sender_clone = server_sender.clone();
    let read_task = async move {
        let mut buf = [0u8; 1024];
        loop {
            match socket_clone.recv(&mut buf).await {
                Ok(len) => {
                    let msg = String::from_utf8_lossy(&buf[..len]);
                    match serde_json::from_str::<ServerEvent>(&msg) {
                        Ok(event) => {
                            let _ = server_sender_clone.send(event).await;
                        }
                        Err(e) => {
                            error!("Failed to parse UDP server event: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to receive UDP message: {:?}", e);
                    break;
                }
            }
        }
    };

    let write_task = async move {
        while let Ok(event) = client_receiver.recv().await {
            let msg = serde_json::to_string(&event)? + "\n";
            socket.send(msg.as_bytes()).await?;
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    };

    task_pool.spawn(read_task).detach();
    write_task.await?;

    Ok(())
}
