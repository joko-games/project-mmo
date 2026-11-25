//! `--capture <file.png>`: connects, waits a moment in the game, saves a screenshot and quits.
//! For checking what the game looks like without anyone at the keyboard. With `--in-core`, the
//! core is out of the structure by then, and the view is the core's.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

use crate::probe::{Core, CoreState, CoreView, Piloting, Probe};
use crate::{Screen, Session};

/// Seconds into the session when the picture is taken.
const AFTER: f64 = 9.0;

#[derive(Resource)]
pub struct Capture(pub Option<String>);

/// `--in-core`: still to do, or done.
#[derive(Resource)]
pub struct CaptureInCore(pub bool);

pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (go_into_core, capture).run_if(in_state(Screen::Playing)),
        );
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

/// Two seconds in, the core leaves the structure and floats off to one side, looking back at it.
fn go_into_core(
    time: Res<Time>,
    session: Res<Session>,
    mut wanted: ResMut<CaptureInCore>,
    mut piloting: ResMut<Piloting>,
    mut view: ResMut<CoreView>,
    probe: Query<&Transform, (With<Probe>, Without<Core>)>,
    mut cores: Query<(&mut Core, &mut Transform), Without<Probe>>,
) {
    if !wanted.0 || time.elapsed_secs_f64() - session.started < 2.0 {
        return;
    }
    let (Ok(structure), Ok((mut core, mut transform))) = (probe.single(), cores.single_mut())
    else {
        return;
    };
    wanted.0 = false;
    *piloting = Piloting::Core;
    core.state = CoreState::Free;
    transform.translation = structure.translation + structure.rotation * Vec3::new(4.0, 1.5, -5.0);
    view.yaw = 2.2;
    view.pitch = -0.25;
    view.distance = 6.0;
}
