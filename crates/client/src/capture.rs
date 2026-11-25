//! `--capture <file.png>`: connects, waits a moment in the game, saves a screenshot and quits.
//! For checking what the game looks like without anyone at the keyboard.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

use crate::{Screen, Session};

/// Seconds into the session when the picture is taken.
const AFTER: f64 = 9.0;

#[derive(Resource)]
pub struct Capture(pub Option<String>);

pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, capture.run_if(in_state(Screen::Playing)));
    }
}

fn capture(
    mut commands: Commands,
    mut wanted: ResMut<Capture>,
    time: Res<Time>,
    session: Res<Session>,
) {
    if time.elapsed_secs_f64() - session.started < AFTER {
        return;
    }
    let Some(path) = wanted.0.take() else {
        return;
    };
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path))
        .observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
}
