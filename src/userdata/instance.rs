use crate::{core::{WorldAccess, lua::LuaSingleton, object::{instance::INSTANCE_CONSTRUCTOR, OBJECT_VTABLES}}, userdata::ObjectRef};
use bevy::prelude::*;
use mlua::prelude::*;
use crate::internal_prelude::*;
use bevy_rblx_derive::register;


pub fn instance_protected_new(
    lua: &Lua,
    (class_name, parent): (String, Option<ObjectRef>),
) -> LuaResult<ObjectRef> {
    let e = {
        let world = WorldAccess::fetch_readonly(lua);
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
    let vtable = *OBJECT_VTABLES.get(class_name.as_str()).unwrap();
    for vtable in vtable.method_resolution_order.iter().copied() {
        if let Some(post_init) = vtable.post_init {
            post_init(lua, e)?;
        }
    }
    Ok(ObjectRef::new(lua, e))
}

pub fn instance_new(lua: &Lua, class_name: String) -> LuaResult<ObjectRef> {
    let e = {
        let mut wa = WorldAccess::fetch(lua);
        wa.assert_synchronized()?;
        let mut commands = wa.access_commands();
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
    let vtable = *OBJECT_VTABLES.get(class_name.as_str()).unwrap();
    for vtable in vtable.method_resolution_order.iter().copied().rev() {
        if let Some(post_init) = vtable.post_init {
            post_init(lua, e)?;
        }
    }
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
