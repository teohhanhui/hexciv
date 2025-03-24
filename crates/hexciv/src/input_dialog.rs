use bevy::ecs::system::SystemId;
use bevy::prelude::*;
use bevy_simple_text_input::{
    TextInputBundle, TextInputPlugin, TextInputSubmitEvent, TextInputSystem,
};

use crate::asset::FontHandle;
use crate::state::InputDialogState;

const BORDER_COLOR_ACTIVE: Color = Color::srgb(0.75, 0.52, 0.99);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
const BACKGROUND_COLOR: Color = Color::srgb(0.15, 0.15, 0.15);

#[derive(Resource)]
pub struct InputDialog(pub Entity);

#[derive(Resource)]
pub struct InputDialogValue(pub String);

#[derive(Resource)]
pub struct InputDialogCallback(pub SystemId);

pub struct InputDialogPlugin;

impl Plugin for InputDialogPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TextInputPlugin)
            .add_systems(OnEnter(InputDialogState::Shown), show_input_dialog)
            .add_systems(
                OnEnter(InputDialogState::Hidden),
                hide_input_dialog.run_if(resource_exists::<InputDialog>),
            )
            .add_systems(
                Update,
                handle_text_input_submit
                    .after(TextInputSystem)
                    .run_if(on_event::<TextInputSubmitEvent>()),
            );
    }
}

fn show_input_dialog(mut commands: Commands, font_handle: Res<FontHandle>) {
    let parent_entity = commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..Default::default()
            },
            ..Default::default()
        })
        .with_children(|parent| {
            parent.spawn((
                NodeBundle {
                    style: Style {
                        width: Val::Vw(80.0),
                        max_width: Val::VMin(100.0),
                        border: UiRect::all(Val::Px(5.0)),
                        padding: UiRect::all(Val::Px(5.0)),
                        ..Default::default()
                    },
                    border_color: BORDER_COLOR_ACTIVE.into(),
                    background_color: BACKGROUND_COLOR.into(),
                    ..Default::default()
                },
                TextInputBundle::default().with_text_style(TextStyle {
                    font: font_handle.clone(),
                    font_size: 40.0,
                    color: TEXT_COLOR,
                }),
            ));
        })
        .id();

    commands.insert_resource(InputDialog(parent_entity));
}

fn hide_input_dialog(mut commands: Commands, input_dialog: Res<InputDialog>) {
    commands.entity(input_dialog.0).despawn_recursive();
    commands.remove_resource::<InputDialog>();
}

fn handle_text_input_submit(
    mut commands: Commands,
    input_dialog_callback: Option<Res<InputDialogCallback>>,
    mut text_input_submit_events: EventReader<TextInputSubmitEvent>,
) {
    for text_input_submit in text_input_submit_events.read() {
        let TextInputSubmitEvent { value, .. } = text_input_submit;
        debug!(?value, "text input submit");
        commands.insert_resource(InputDialogValue(value.clone()));
        if let Some(input_dialog_callback) = &input_dialog_callback {
            commands.run_system(input_dialog_callback.0);
        }
    }
}
