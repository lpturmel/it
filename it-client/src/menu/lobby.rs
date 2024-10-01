use crate::cleanup_entities;
use bevy::prelude::*;

use super::{GenericButton, MenuState};

pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MenuState::Lobby), setup_lobby)
            .add_systems(Update, menu_interaction.run_if(in_state(MenuState::Lobby)))
            .add_systems(OnExit(MenuState::Lobby), cleanup_entities::<OnLobbyScreen>);
    }
}

#[derive(Component)]
enum LobbyButtonAction {
    Leave,
}

#[derive(Component)]
struct OnLobbyScreen;

fn menu_interaction(
    interaction_query: Query<
        (&Interaction, &LobbyButtonAction),
        (Changed<Interaction>, With<Button>),
    >,
    mut menu_state: ResMut<NextState<MenuState>>,
) {
    for (interaction, button_action) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match button_action {
                LobbyButtonAction::Leave => {
                    menu_state.set(MenuState::Main);
                }
            }
        }
    }
}
fn setup_lobby(mut commands: Commands) {
    // Root node
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    flex_direction: FlexDirection::Column,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                background_color: Color::BLACK.into(),
                ..default()
            },
            OnLobbyScreen,
        ))
        .with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "Waiting for players...",
                TextStyle {
                    font_size: 40.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
            parent.spawn(GenericButton::new("Leave Game", LobbyButtonAction::Leave));
        });
}
