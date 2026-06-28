use crate::core::Engine;

#[test]
pub fn parent_change_before_groups() {
    let mut app = Engine::test_mode(Some(2));
    Engine::test_mode_lua_load(
        &mut app,
        r#"
        local m = Instance.new("ModuleScript")
        m.Parent = workspace
        local f = Instance.new("Folder")
        f.Parent = game:GetService("ReplicatedStorage")
        f.Name = "Modules"
        m.Parent = f

        f = nil
        m = nil
        
        task.defer(coroutine.running())
        coroutine.yield()
        assert(game.ReplicatedStorage.Modules ~= nil, "modules exist")
        assert(game.ReplicatedStorage.Modules.ModuleScript ~= nil, "modulescript exists")
    "#,
    );
    app.run();
}
