use crate::internal_prelude::*;
use crate::{
    core::{
        WorldAccess,
        lua::LuaSingleton,
        object::{OBJECT_VTABLES, instance::INSTANCE_CONSTRUCTOR},
    },
    userdata::ObjectRef,
};
use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::prelude::*;

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
            bevy::log::trace!(target: "bevy_rblx::instance_protected_new", "Spawning instance {} with class {class_name}", entity.id());
            if parent.is_some() {
                entity.insert(ChildOf(parent.unwrap().entity()));
            }
            entity.id()
        } else {
            bevy::log::error!(target: "bevy_rblx::instance_protected_new", "Failed spawning instance {} with class {class_name}", entity.id());
            entity.despawn(); // abort
            res?;
            unreachable!()
        }
    };
    let vtable = *OBJECT_VTABLES.get(class_name.as_str()).unwrap();
    for vtable in vtable.method_resolution_order.iter().copied() {
        if let Some(post_init) = vtable.post_init {
            if let Err(err) = post_init(lua, e) {
                // The ref counted system will automatically delete the object if this fails
                bevy::log::error!(target: "bevy_rblx::instance_protected_new", "Failed spawning instance {e} with class {class_name}");
                return Err(err);
            }
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
            bevy::log::debug!(target: "bevy_rblx::instance_new", "Spawning instance {} with class {class_name}", entity.id());
            entity.id()
        } else {
            bevy::log::error!(target: "bevy_rblx::instance_new", "Failed spawning instance {} with class {class_name}", entity.id());
            entity.despawn(); // abort
            res?;
            unreachable!()
        }
    };
    let vtable = *OBJECT_VTABLES.get(class_name.as_str()).unwrap();
    for vtable in vtable.method_resolution_order.iter().copied().rev() {
        if let Some(post_init) = vtable.post_init {
            if let Err(err) = post_init(lua, e) {
                // The ref counted system will automatically delete the object if this fails
                bevy::log::error!(target: "bevy_rblx::instance_new", "Failed spawning instance {e} with class {class_name}");
                return Err(err);
            }
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
