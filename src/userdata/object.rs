use std::ops::Deref;

use crate::internal_prelude::*;
use bevy::prelude::*;
use mlua::prelude::*;

use bevy::ecs::entity::Entity;

use crate::core::{object::ObjectHeader, world_access::WorldAccess};

pub struct ObjectRef(Entity, WeakLua);

impl FromLua for ObjectRef {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        let v: LuaUserDataRef<Self> = value.borrow_typed()?;
        unsafe {
            WorldAccess::fetch(lua)
                .access_commands()
                .entity(v.0)
                .inc_ref()
        };
        Ok(Self(v.0, lua.weak()))
    }
}

impl ObjectRef {
    pub fn new(lua: &Lua, e: Entity) -> ObjectRef {
        unsafe {
            WorldAccess::fetch_readonly(lua)
                .access_commands()
                .entity(e)
                .inc_ref()
        };
        ObjectRef(e, lua.weak())
    }
    pub fn entity(&self) -> Entity {
        self.0
    }
    pub fn clone_lua(&self, lua: &Lua) -> Self {
        let wa = WorldAccess::fetch_readonly(lua);
        let mut commands = wa.access_commands();

        unsafe {
            commands.entity(self.0).inc_ref();
        }
        Self {
            0: self.0,
            1: lua.weak()
        }
    }
    pub unsafe fn clone_no_inc_ref(&self) -> Self {
        Self {
            0: self.0,
            1: self.1.clone()
        }
    }
}

impl Deref for ObjectRef {
    type Target = Entity;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for ObjectRef {
    fn drop(&mut self) {
        unsafe {
            WorldAccess::fetch_readonly(&self.1.upgrade())
                .access_commands()
                .entity(self.0)
                .dec_ref()
        };
    }
}

impl LuaUserData for ObjectRef {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", move |l, t, ()| -> LuaResult<String> {
            let world_access = WorldAccess::fetch_readonly(l);
            let world = world_access.access_read_only();
            let string = world
                .get::<Name>(t.0)
                .map(|x| x.as_str())
                .unwrap_or_else(|| world.get::<ObjectHeader>(t.0).unwrap().vtable.class_name)
                .to_owned();
            Ok(string)
        });
        methods.add_meta_method(
            "__index",
            move |l, t, (k,): (String,)| -> LuaResult<LuaValue> {
                let vtable = WorldAccess::fetch_readonly(l)
                    .access_read_only()
                    .get::<ObjectHeader>(t.0)
                    .unwrap()
                    .vtable;
                vtable.get(l, t.0, k)
            },
        );
        methods.add_meta_method(
            "__newindex",
            move |l, t, (k, v): (String, LuaValue)| -> LuaResult<()> {
                let vtable = WorldAccess::fetch_readonly(l)
                    .access_read_only()
                    .get::<ObjectHeader>(t.0)
                    .unwrap()
                    .vtable;
                vtable.set(l, t.0, k, v)
            },
        );
    }
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "Object")
    }
}

impl Clone for ObjectRef {
    fn clone(&self) -> Self {
        let lua = self.1.upgrade();
        let world_access = WorldAccess::fetch_readonly(&lua);
        let mut commands = world_access.access_commands();

        unsafe {
            commands.entity(self.entity()).inc_ref();
        }
        Self(self.0.clone(), self.1.clone())
    }
}