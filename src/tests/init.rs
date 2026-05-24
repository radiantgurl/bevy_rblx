use bevy::{
    app::PostStartup,
    ecs::{entity::Entity, hierarchy::ChildOf, query::With, world::World},
};

use crate::core::{
    Engine, LuauContainer,
    object::{RootInstance, RunServiceMembers},
};

#[test]
pub fn server_initialization_stability() {
    let mut app = Engine::test_mode(6000);
    app.run();
}

#[test]
pub fn print_core_initialized_table() {
    let mut app = Engine::test_mode(2);
    app.add_systems(PostStartup, post_startup_hook);
    fn post_startup_hook(w: &mut World) {
        let lua = {
            let game = w
                .query_filtered::<Entity, With<RootInstance>>()
                .single(w)
                .unwrap();
            w.get::<LuauContainer>(game).unwrap().lua.clone()
        };
        lua.load(
            r#"
        task.defer(function()
            game:DebugPrintTree()
        end)"#,
        )
        .exec()
        .unwrap();
    }
    app.run();
}

#[test]
pub fn object_postinit() {
    let mut app = Engine::test_mode(2);
    app.add_systems(PostStartup, post_startup_hook);
    fn post_startup_hook(w: &mut World) {
        let run_service = w
            .query_filtered::<Entity, With<RunServiceMembers>>()
            .single(w)
            .unwrap();
        w.get::<ChildOf>(run_service).expect("Expected Run service is parented to game instance, are you sure object's postinit is working properly?");
    }
    app.run();
}
