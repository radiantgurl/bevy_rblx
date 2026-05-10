use crate::userdata::ObjectRef;
use crate::{self as bevy_rblx, core::instance::INSTANCE_CONSTRUCTOR};
use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::prelude::*;

use crate::core::{LuaSingleton, world_access::WorldAccess};

pub fn instance_protected_new(
    lua: &Lua,
    (class_name, parent): (String, Option<ObjectRef>),
) -> LuaResult<ObjectRef> {
    let world = WorldAccess::fetch_readonly(lua);

    let e = {
        let mut commands = world.access_commands();
        let mut entity = commands.spawn_empty();
        let res = INSTANCE_CONSTRUCTOR.protected_new(lua, entity.reborrow(), &class_name);

        if res.is_ok() {
            if parent.is_some() {
                entity.insert(ChildOf(parent.unwrap().entity()));
            }
            entity.id()
        } else {
            entity.despawn(); // abort
            res?;
            unreachable!()
        }
    };
    Ok(ObjectRef::new(lua, e))
}

pub fn instance_new(lua: &Lua, class_name: String) -> LuaResult<ObjectRef> {
    let e = {
        let world = WorldAccess::fetch_readonly(lua);
        let mut commands = world.access_commands();
        let mut entity = commands.spawn_empty();
        let res = INSTANCE_CONSTRUCTOR.new(lua, entity.reborrow(), &class_name);

        if res.is_ok() {
            entity.id()
        } else {
            entity.despawn(); // abort
            res?;
            unreachable!()
        }
    };
    Ok(ObjectRef::new(lua, e))
}

pub struct InstanceSingleton;

#[register]
impl LuaSingleton for InstanceSingleton {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        let instance = lua.create_table()?;

        instance.raw_set("new", lua.create_function(instance_protected_new)?)?;

        instance.set_readonly(true);

        lua.globals().raw_set("Instance", instance)
    }
}
