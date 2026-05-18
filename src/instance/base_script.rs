use bevy::ecs::entity::Entity;
use bevy_rblx_derive::register_class;
use mlua::prelude::*;

use crate::core::{InstanceMembers, LuaSingleton};
use crate::enums::RunContext;
use crate::internal_prelude::*;

use crate::core::WorldAccess;

register_class! {
    abstract LuaSourceContainer (Instance)
    members {
        #[security=PLUGIN]
        pub source: String
    }
    methods {}
}

pub struct ModuleScriptTable(HashMap<Entity, LuaValue>);
pub struct Thread

fn set_enabled(lua: &Lua, this: Entity, new_value: bool) -> LuaResult<bool> {
    let mut wa = WorldAccess::fetch(lua);
    let world = wa.access_synchronized()?;
    {
        let mut members = BaseScriptMembers::fetch_members_mut(world, this);
        if members.enabled == new_value {
            return Ok(false);
        }
        members.enabled = new_value;
    }
    if new_value {
        
    } else {

    }
}

register_class! {
    abstract BaseScript (LuaSourceContainer)
    members {
        #[setter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable, value: LuaValue) -> LuaResult<bool> {
            let new_value = bool::from_lua(value, lua)?;
            set_enabled(lua, this, new_value)
        }]
        enabled: bool,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();

            world.get::<BaseScriptMembers>(this).unwrap().enabled.into_lua(lua)
        }]
        #[setter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable, value: LuaValue) -> LuaResult<bool> {
            let new_value = bool::from_lua(value, lua)?;
            set_enabled(lua, this, !new_value)
        }]
        virtual disabled: bool,
        pub run_context: RunContext
    }
    methods {}
}
