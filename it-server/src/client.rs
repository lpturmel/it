use it_core::{ClientEvent, IntoResponse, LeaveEvent, PosUpdateEvent, ServerEvent, StartEvent};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpStream, UdpSocket};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let stream = TcpStream::connect("127.0.0.1:8080").await?;
    info!("Connected to server");

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    let event = ClientEvent::Join;
    let event = event.into_response();
    writer.write_all(event.as_bytes()).await?;

    // Create a UDP socket
    let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
    udp_socket.connect("127.0.0.1:8081").await?;
    info!("Connected to server via UDP");

    let mut client_id = String::new();

    let udp_socket_reader = udp_socket.clone();
    let udp_socket_writer = udp_socket.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            match udp_socket_reader.recv(&mut buf).await {
                Ok(len) => {
                    let msg = String::from_utf8_lossy(&buf[..len]);
                    info!("Received UDP message: {}", msg);
                }
                Err(e) => {
                    error!("UDP receive error: {}", e);
                    break;
                }
            }
        }
    });

    while reader.read_line(&mut line).await? != 0 {
        let response = line.trim();

        let event = serde_json::from_str::<ServerEvent>(response)?;

        match event {
            ServerEvent::Wait => {
                info!("Waiting for players...");
            }
            ServerEvent::Start(StartEvent {
                lobby_id,
                client_id,
                players,
            }) => {
                info!(
                    "Starting the game... Lobby: {}\nPlayers:{}",
                    lobby_id,
                    players.len()
                );
                let udp_socket_writer = udp_socket_writer.clone();

                let client_id = client_id.clone();
                tokio::spawn(async move {
                    let mut x = 0.0f32;
                    let mut y = 0.0f32;

                    loop {
                        // Simulate position update
                        x += 1.0;
                        y += 1.0;

                        let event = ClientEvent::PosUpdate(PosUpdateEvent {
                            client_id: client_id.clone(),
                            x,
                            y,
                        });
                        let position_update = event.into_response();
                        if let Err(e) = udp_socket_writer.send(position_update.as_bytes()).await {
                            error!("Failed to send UDP message: {}", e);
                            break;
                        }

                        info!("Sent position update: x={}, y={}", x, y);

                        // Sleep for a while before sending the next update
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                });
            }
            ServerEvent::Leave(LeaveEvent { client_id }) => {
                info!("Player {} left the game", client_id);
            }
        }

        line.clear();
    }

    Ok(())
}
