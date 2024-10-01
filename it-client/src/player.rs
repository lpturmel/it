use crate::net::UdpSocketSender;
use crate::GameState;
use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use bevy::window::PrimaryWindow;
use bevy_rapier2d::prelude::*;
use it_core::PosUpdateEvent;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentPlayerPos>()
            .observe(spawn_player)
            .add_systems(
                Update,
                (
                    wrap_player_position,
                    main_player_inputs,
                    broadcast_main_player_pos,
                )
                    .run_if(in_state(GameState::Game)),
            );
    }
}

#[derive(Event)]
pub struct SpawnPlayerEvent {
    pub coords: Vec2,
    pub id: String,
    pub main_player: bool,
}

#[derive(Component)]
pub struct Player {
    pub id: String,
}

#[derive(Component)]
struct MainPlayer;

#[derive(Bundle)]
struct PlayerBundle {
    name: Name,
    player: Player,
    sprite: SpriteBundle,
    rigid_body: RigidBody,
    locked_axes: LockedAxes,
    collider: Collider,
    velocity: Velocity,
    texture: TextureAtlas,
}

#[derive(Resource, Default)]
pub struct CurrentPlayerPos {
    pub position: Option<Vec3>,
}

fn spawn_player(
    trigger: Trigger<SpawnPlayerEvent>,
    mut commands: Commands,
    mut textures: ResMut<Assets<TextureAtlasLayout>>,
    asset_server: Res<AssetServer>,
    mut rapier_config: ResMut<RapierConfiguration>,
) {
    rapier_config.gravity = Vec2::ZERO;
    let texture_handle = asset_server.load("slime.png");

    let layout = TextureAtlasLayout::from_grid(UVec2::new(32, 32), 7, 13, None, None);
    let texture_atlas_handle = textures.add(layout);

    let coords = trigger.event().coords;
    let coords = Vec3::new(coords.x, coords.y, 0.0);

    let player_name = format!("Player-{}", trigger.event().id);

    let player_bundle = PlayerBundle {
        name: Name::new(player_name.clone()),
        player: Player {
            id: trigger.event().id.clone(),
        },
        texture: TextureAtlas {
            index: 0,
            layout: texture_atlas_handle,
        },
        sprite: SpriteBundle {
            texture: texture_handle,
            transform: Transform {
                translation: coords,
                scale: Vec3::splat(1.5),
                ..default()
            },
            ..default()
        },
        locked_axes: LockedAxes::ROTATION_LOCKED,
        rigid_body: RigidBody::Dynamic,
        collider: Collider::cuboid(8.0, 8.0),
        velocity: Velocity::default(),
    };
    let entity = commands.spawn(player_bundle).id();
    if trigger.event().main_player {
        commands.entity(entity).insert(MainPlayer);
    }
    commands.entity(entity).with_children(|p| {
        p.spawn(Text2dBundle {
            text: Text::from_section(
                player_name,
                TextStyle {
                    font_size: 11.0,
                    color: Color::WHITE,
                    ..default()
                },
            ),
            transform: Transform::from_xyz(0.0, 15.0, 0.0),
            ..default()
        });
    });
}
fn wrap_player_position(
    mut query: Query<&mut Transform, With<Player>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let window = windows.single();
    let half_width = window.width() / 2.0;
    let half_height = window.height() / 2.0;

    for mut transform in query.iter_mut() {
        let mut position = transform.translation;

        if position.x > half_width {
            position.x = -half_width;
        } else if position.x < -half_width {
            position.x = half_width;
        }

        if position.y > half_height {
            position.y = -half_height;
        } else if position.y < -half_height {
            position.y = half_height;
        }

        transform.translation = position;
    }
}

fn main_player_inputs(
    mut query: Query<&mut Velocity, With<MainPlayer>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    let mut velocity = query.single_mut();

    let speed = 150.0;

    let mut direction = Vec2::ZERO;

    if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    if direction.length_squared() > 0.0 {
        direction = direction.normalize();
        velocity.linvel = direction * speed;
    } else {
        velocity.linvel = Vec2::ZERO;
    }
}

fn broadcast_main_player_pos(
    mut last_pos: ResMut<CurrentPlayerPos>,
    socket_sender: ResMut<UdpSocketSender>,
    player_q: Query<(&Transform, &Player), With<MainPlayer>>,
) {
    let (transform, player) = player_q.single();
    let coords = transform.translation;

    if let Some(last_pos) = last_pos.position {
        if coords == last_pos {
            return;
        }
    }

    let task_pool = IoTaskPool::get();
    let socket_sender = socket_sender.0.clone();
    let player_id = player.id.clone();
    task_pool
        .spawn(async move {
            let _ = socket_sender
                .send(it_core::ClientEvent::PosUpdate(PosUpdateEvent {
                    x: coords.x,
                    y: coords.y,
                    client_id: player_id,
                }))
                .await;
        })
        .detach();
    last_pos.position = Some(coords);
}
