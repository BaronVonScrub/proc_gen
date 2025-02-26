use bevy::app::Plugin;
use bevy::input::ButtonState;
use bevy::input::mouse::{MouseButtonInput};
use bevy::prelude::*;

pub(crate) struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<InputStates>()
            .add_systems(Update,input_management_system);
    }
}

#[derive(Debug, Default)]
pub enum MouseButtonState {
    #[default]
    Unheld,
    Held(Vec2),
}

#[derive(Default, Resource)]
pub(crate) struct InputStates {
    pub left: MouseButtonState,
    pub right: MouseButtonState,
    pub middle: MouseButtonState,
    pub cursor_position: Vec2,
}

fn input_management_system(
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut mouse_button_input_events: EventReader<MouseButtonInput>,
    mut input: ResMut<InputStates>,
) {
    // Track the cursor position
    for event in cursor_moved_events.read() {
        input.cursor_position = event.position;
    }

    for event in mouse_button_input_events.read() {
        match event.button {
            MouseButton::Left => {
                input.left = match event.state {
                    ButtonState::Pressed => MouseButtonState::Held(input.cursor_position),
                    ButtonState::Released => MouseButtonState::Unheld,
                };
            }
            MouseButton::Right => {
                input.right = match event.state {
                    ButtonState::Pressed => MouseButtonState::Held(input.cursor_position),
                    ButtonState::Released => MouseButtonState::Unheld,
                };
            }
            MouseButton::Middle => {
                input.middle = match event.state {
                    ButtonState::Pressed => MouseButtonState::Held(input.cursor_position),
                    ButtonState::Released => MouseButtonState::Unheld,
                };
            }
            _ => {}
        }
    }
}