use crate::internal_prelude::*;
use crate::{
    self as bevy_rblx,
    core::instance::{INSTANCE_CONSTRUCTOR, InstanceConstructor},
};
use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::prelude::*;

use bevy::ecs::entity::Entity;

use crate::core::{LuaSingleton, object::ObjectHeader, world_access::WorldAccess};

pub struct InstanceRef(Entity, Lua);

impl FromLua for InstanceRef {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        let v: LuaUserDataRef<Self> = value.borrow_typed()?;
        WorldAccess::fetch(lua)
            .access_commands()
            .entity(v.0)
            .inc_ref();
        Ok(Self(v.0, lua.clone()))
    }
}

impl InstanceRef {
    pub fn new(lua: &Lua, e: Entity) -> InstanceRef {
        WorldAccess::fetch(lua)
            .access_commands()
            .entity(e)
            .inc_ref();
        InstanceRef(e, lua.clone())
    }
}

impl Drop for InstanceRef {
    fn drop(&mut self) {
        WorldAccess::fetch(&self.1)
            .access_commands()
            .entity(self.0)
            .dec_ref();
    }
}

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
        methods.add_meta_method(
            "__index",
            move |l, t, (k,): (String,)| -> LuaResult<LuaValue> {
                let vtable = WorldAccess::fetch(l).get::<ObjectHeader>(t.0).unwrap().0;
                (vtable.index)(l, t.0, k)
            },
        );
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

fn instance_new(
    lua: &Lua,
    (class_name, parent): (String, Option<InstanceRef>),
) -> LuaResult<InstanceRef> {
    let mut world = WorldAccess::fetch(lua);
    let mut commands = world.access_commands();
    let mut entity = commands.spawn_empty();
    let res = INSTANCE_CONSTRUCTOR.protected_new(lua, entity.reborrow(), &class_name);

    if res.is_ok() {
        if parent.is_some() {
            entity.insert(ChildOf(parent.unwrap().0));
        }
        let e = entity.id();
        world
            .access_synchronized()
            .expect("todo!(): fix for desynchronzied")
            .flush();
        Ok(InstanceRef::new(lua, e))
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

        instance.raw_set("new", lua.create_function(instance_new)?)?;

        instance.set_readonly(true);

        lua.globals().raw_set("Instance", instance)
    }
}
