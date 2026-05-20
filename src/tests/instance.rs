use bevy::{
    app::PostStartup,
    ecs::{entity::Entity, query::With, world::World},
};

use crate::core::{Engine, LuauContainer, instance::RootInstance};

#[test]
pub fn simple_creation_and_deletion() {
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
            local folder = Instance.new("Folder")
            local new_folder = Instance.new("Folder")
            local set_v = 0
            folder.Destroying:Connect(function() print("Destroying Folder1") set_v = 1 end)
            new_folder.Destroying:Connect(function() print("Destroying Folder2") set_v = 1 end)

            new_folder.Parent = folder
            print(new_folder.Parent)
            assert(new_folder.Parent == folder, "parent property works")

            new_folder.Name = "uwu x3"
            assert(new_folder.Name == "uwu x3", "name property works")

            folder.Parent = game
            folder.Name = "mroew moew :3"

            game:DebugPrintTree()
            folder:Destroy()
            game:DebugPrintTree()

            assert(set_v == 1, "Destroying event is emitted")
        end)"#,
        )
        .exec()
        .unwrap();
    }
    app.run();
}
