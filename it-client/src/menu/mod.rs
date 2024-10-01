use crate::net::TcpSocketSender;
use crate::{cleanup_entities, GameState};
use bevy::ecs::component::{ComponentHooks, StorageType};
use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use it_core::ClientEvent;

pub mod lobby;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<MenuState>()
            .add_plugins(lobby::LobbyPlugin)
            .add_systems(OnEnter(GameState::Menu), setup_menu)
            .add_systems(OnEnter(MenuState::Main), main_menu_setup)
            .add_systems(Update, menu_interaction.run_if(in_state(MenuState::Main)))
            .add_systems(
                OnExit(MenuState::Main),
                cleanup_entities::<OnMainMenuScreen>,
            );
    }
}
#[derive(States, Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub enum MenuState {
    #[default]
    Main,
    Lobby,
    Disabled,
}

#[derive(Component)]
pub struct OnMainMenuScreen;

#[derive(Component)]
enum MenuButtonAction {
    Play,
    Quit,
}

fn menu_interaction(
    interaction_query: Query<
        (&Interaction, &MenuButtonAction),
        (Changed<Interaction>, With<Button>),
    >,
    mut socket_sender: ResMut<TcpSocketSender>,
    mut menu_state: ResMut<NextState<MenuState>>,
    mut exit: EventWriter<AppExit>,
) {
    let task_pool = IoTaskPool::get();
    for (interaction, button_action) in &interaction_query {
        let socket_sender = socket_sender.0.clone();
        if *interaction == Interaction::Pressed {
            match button_action {
                MenuButtonAction::Play => {
                    task_pool
                        .spawn(async move {
                            let _ = socket_sender.send(ClientEvent::Join).await;
                        })
                        .detach();
                    menu_state.set(MenuState::Lobby);
                }
                MenuButtonAction::Quit => {
                    exit.send(AppExit::Success);
                }
            }
        }
    }
}
fn main_menu_setup(mut commands: Commands) {
    // Root node
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                background_color: Color::BLACK.into(),
                ..default()
            },
            OnMainMenuScreen,
        ))
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Start,
                        align_items: AlignItems::Center,
                        position_type: PositionType::Absolute,
                        width: Val::Px(30.0),
                        height: Val::Px(30.0),
                        bottom: Val::Px(10.0),
                        right: Val::Percent(5.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn(TextBundle::from_section(
                        format!("v{}", env!("CARGO_PKG_VERSION")),
                        TextStyle {
                            font_size: 20.0,
                            color: Color::WHITE,
                            ..default()
                        },
                    ));
                });
            // Column for buttons
            parent
                .spawn(NodeBundle {
                    style: Style {
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceEvenly,
                        align_items: AlignItems::Center,
                        width: Val::Percent(30.0),
                        height: Val::Percent(30.0),
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn(GenericButton::new("Play", MenuButtonAction::Play));
                    parent.spawn(GenericButton::new("Quit", MenuButtonAction::Quit));
                });
        });
}
fn setup_menu(mut menu_state: ResMut<NextState<MenuState>>) {
    menu_state.set(MenuState::Main);
}

#[derive(Bundle)]
pub struct GenericButton<T: Component> {
    button_bundle: ButtonBundle,
    text_bundle: BundleChild<TextBundle>,
    component: T,
}

impl<T: Component> GenericButton<T> {
    pub fn new<S>(text: S, marker: T) -> Self
    where
        S: Into<String>,
    {
        Self {
            button_bundle: ButtonBundle {
                style: Style {
                    width: Val::Px(150.0),
                    height: Val::Px(65.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                background_color: Color::WHITE.into(),
                ..default()
            },
            text_bundle: BundleChild::new(TextBundle::from_section(
                text,
                TextStyle {
                    font_size: 40.0,
                    color: Color::BLACK,
                    ..default()
                },
            )),
            component: marker,
        }
    }

    pub fn with_width(mut self, width: Val) -> Self {
        self.button_bundle.style.width = width;
        self
    }
    pub fn with_height(mut self, height: Val) -> Self {
        self.button_bundle.style.height = height;
        self
    }
}
pub struct BundleChild<B: Bundle>(Option<B>);

impl<T: Bundle> BundleChild<T> {
    pub fn new(bundle: T) -> Self {
        Self(Some(bundle))
    }
}

impl<T: Bundle> Component for BundleChild<T> {
    const STORAGE_TYPE: StorageType = StorageType::Table;
    fn register_component_hooks(hooks: &mut ComponentHooks) {
        hooks.on_add(|mut world, entity, _component_id| {
            let bundle = world.get_mut::<BundleChild<T>>(entity).unwrap().0.take();
            if let Some(bundle) = bundle {
                world.commands().entity(entity).with_children(|parent| {
                    parent.spawn(bundle);
                });
            }
        });
    }
}
