use crate::core::Engine;

#[test]
pub fn simple_creation_and_deletion() {
    let mut app = Engine::test_mode(Some(2));
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

#[test]
pub fn ancestry_event_order() {
    let mut app = Engine::test_mode(None);
    Engine::test_mode_test_service_script(
        &mut app,
        "Ancestry Event Order",
        r#"
        local parent = Instance.new("Folder")
        local child = Instance.new("Folder")
        local order = {}

        parent.ChildAdded:Connect(function(_) order[#order+1]="ChildAdded" end)
        parent.DescendantAdded:Connect(function(_) order[#order+1]="DescendantAdded" end)
        parent.ChildRemoved:Connect(function(_) order[#order+1]="ChildRemoved" end)
        parent.DescendantRemoving:Connect(function(_) order[#order+1]="DescendantRemoving" end)
        child.AncestryChanged:Connect(function(_) order[#order+1]="AncestryChanged" end)
        child.Parent = parent
        child.Parent = nil
        for _,v in ipairs(order) do
            print(v)
        end
        RBX_REQUIRE_EQ(order[1], "ChildAdded")
        RBX_REQUIRE_EQ(order[2], "DescendantAdded")
        RBX_REQUIRE_EQ(order[3], "AncestryChanged")
        RBX_REQUIRE_EQ(order[4], "DescendantRemoving")
        RBX_REQUIRE_EQ(order[5], "ChildRemoved")
        RBX_REQUIRE_EQ(order[6], "AncestryChanged")
    "#,
    );
    app.run();
}
