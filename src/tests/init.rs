use bevy::{
    app::{AppExit, Main},
    ecs::{message::MessageWriter, system::Local},
};

use crate::core::Engine;

fn generate_exit_after_60_frames(
    mut frame_count: Local<Option<i32>>,
    mut writer: MessageWriter<AppExit>,
) {
    if frame_count.is_none() {
        *frame_count = Some(0);
    }
    if frame_count.unwrap() == 60 {
        writer.write(AppExit::Success);
    }
    *frame_count.as_mut().unwrap() += 1;
}

#[test]
pub fn server_initialization() {
    let mut app = Engine::headless();
    app.add_systems(Main, generate_exit_after_60_frames);
    app.run();
}

// tenplate <typename T>
// T& get() ...
