use crate::core::Engine;

#[test]
pub fn simple_creation_and_deletion() {
    let mut app = Engine::test_mode(2);
    Engine::test_mode_lua_load(
        &mut app,
        r#"
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
    "#,
    );
    app.run();
}
