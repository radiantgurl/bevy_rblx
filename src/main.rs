pub mod core;
pub mod enums;
pub mod instance;
pub mod userdata;

pub(crate) use core::internal_prelude;

use bevy::prelude::*;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins).run();
}
