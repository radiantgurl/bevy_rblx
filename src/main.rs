pub mod core;
pub mod enums;
pub mod instance;
pub mod userdata;

use bevy::prelude::*;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins).run();
}
