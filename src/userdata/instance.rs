use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::prelude::*;
use crate::{self as bevy_rblx, core::instance::{INSTANCE_CONSTRUCTOR, InstanceConstructor}};

use bevy::ecs::entity::Entity;

use crate::core::{LuaSingleton, object::ObjectHeader, world_access::WorldAccess};

#[derive(Clone, Copy, FromLua)]
pub struct InstanceRef(pub Entity);

impl LuaUserData for InstanceRef {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", move |l, t, ()| -> LuaResult<String> {
            let name = WorldAccess::fetch(l)
                .get::<Name>(t.0)
                .unwrap()
                .as_str()
                .to_owned();
            Ok(name)
        });
        methods.add_meta_method("__index", move |l, t, (k,): (String,)| -> LuaResult<LuaValue> {
            let vtable = WorldAccess::fetch(l).get::<ObjectHeader>(t.0).unwrap().0;
            (vtable.index)(l, t.0, k)
        });
        methods.add_meta_method(
            "__newindex",
            move |l, t, (k, v): (String, LuaValue)| -> LuaResult<()> {
                let vtable = WorldAccess::fetch(l).get::<ObjectHeader>(t.0).unwrap().0;
                if let Some(f) = vtable.newindex {
                    f(l, t.0, k, v)
                } else {
                    Err(LuaError::runtime(format!(
                        "object of type {} has no setter",
                        vtable.class_name
                    )))
                }
            },
        );
    }
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "Instance")
    }
}

fn instance_new(l: &Lua, (class_name, parent): (String, Option<InstanceRef>)) -> LuaResult<InstanceRef> {
    let mut world = WorldAccess::fetch(l);
    let mut commands = world.commands();
    let mut entity = commands.spawn_empty();
    let res = INSTANCE_CONSTRUCTOR.protected_new(l, entity.reborrow(), &class_name);

    if res.is_ok() {
        if parent.is_some() {
            entity.insert(ChildOf(parent.unwrap().0));
        }
        let e = entity.id();
        world.flush();
        Ok(InstanceRef(e))
    } else {
        entity.despawn(); // abort
        res?;
        unreachable!();
    }
}

#[register]
impl LuaSingleton for InstanceRef {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        let instance = lua.create_table()?;

        instance.raw_set("new",lua.create_function(instance_new)?)?;

        lua.globals().raw_set("Instance", instance)
    }
}