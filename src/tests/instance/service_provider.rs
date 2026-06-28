use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use bevy::{
    app::PostStartup,
    ecs::{entity::Entity, query::With, resource::Resource, world::World},
};

use crate::core::{Engine, LuauContainer, object::RootInstance};

#[test]
pub fn bind_to_close() {
    let mut app = Engine::test_mode(Some(4));
    app.add_systems(PostStartup, post_startup_hook);
    #[derive(Resource, Clone)]
    struct FlagRes(Arc<AtomicBool>);
    let flag = Arc::new(AtomicBool::new(false));
    app.world_mut().insert_resource(FlagRes(flag.clone()));
    fn post_startup_hook(w: &mut World) {
        let lua = {
            let game = w
                .query_filtered::<Entity, With<RootInstance>>()
                .single(w)
                .unwrap();
            w.get::<LuauContainer>(game).unwrap().lua.clone()
        };
        let flag = w.resource::<FlagRes>().0.clone();
        lua.globals()
            .raw_set(
                "on_close",
                lua.create_function(move |_, ()| {
                    flag.store(true, Ordering::Relaxed);
                    Ok(())
                })
                .unwrap(),
            )
            .unwrap();
        lua.load(
            r#"
        task.defer(function()
            game.Close:Connect(on_close)
        end)"#,
        )
        .exec()
        .unwrap();
    }
    app.run();
    assert!(
        flag.load(Ordering::Relaxed),
        "Post Close event flag was not set, did the event not fire?"
    );
}
