use bevy_rblx_derive::register_class;
use mlua::prelude::*;

use crate::core::InstanceMembers;
use crate::enums::RunContext;
use crate::internal_prelude::*;

use crate::core::WorldAccess;

register_class! {
    abstract LuaSourceContainer (Instance)
    members {}
    methods {}
}

register_class! {
    abstract BaseScript (LuaSourceContainer)
    members {
        enabled: bool,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();

            (!world.get::<BaseScriptMembers>(this).expect("expected base script").enabled).into_lua(lua)
        }]
        virtual disabled: bool,
        pub run_context: RunContext,
        pub source: String
    }
    methods {}
}
