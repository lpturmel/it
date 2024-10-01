use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier2d::prelude::*;
use camera::CameraPlugin;
use menu::MenuPlugin;
use player::PlayerPlugin;

use self::net::NetworkPlugin;

pub mod animation;
pub mod camera;
pub mod game;
pub mod menu;
pub mod net;
pub mod player;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    Menu,
    Game,
}

fn main() {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "It".to_string(),
                    name: Some("main".to_string()),
                    resizable: true,
                    present_mode: bevy::window::PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            }),
        WorldInspectorPlugin::new(),
        RapierPhysicsPlugin::<()>::pixels_per_meter(32.0),
        RapierDebugRenderPlugin::default(),
    ))
    .add_plugins((CameraPlugin, NetworkPlugin, MenuPlugin, PlayerPlugin))
    .init_state::<GameState>()
    .run();
}

pub fn cleanup_entities<T: Component>(to_despawn: Query<Entity, With<T>>, mut commands: Commands) {
    for entity in &to_despawn {
        commands.entity(entity).despawn_recursive();
    }
}
