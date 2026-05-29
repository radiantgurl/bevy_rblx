use std::{ops::Deref, sync::Arc};

use crate::{
    core::{
        WorldAccess,
        bevy::{EntityCommandWrapper, ref_counted::commands::dec_ref_command},
        lua::world_access::WorldAccessDestructor,
        object::ObjectHeader,
    },
    internal_prelude::*,
};
use bevy::prelude::*;
use mlua::prelude::*;

use bevy::ecs::entity::Entity;
use parking_lot::Mutex;

pub struct ObjectRef(Entity, WeakLua, Arc<Mutex<WorldAccessDestructor>>);

impl std::fmt::Debug for ObjectRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ObjectRef").field(&self.0).finish()
    }
}
impl PartialEq for ObjectRef {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl FromLua for ObjectRef {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        let v: LuaUserDataRef<Self> = value.borrow_typed()?;
        unsafe {
            let wa = WorldAccess::fetch_readonly(lua);
            wa.access_commands().entity(v.0).inc_ref();
            // wa.access_read_only().get::<RefCounted>(v.0).unwrap()
        }
        Ok(Self(
            v.0,
            lua.weak(),
            lua.app_data_ref::<Arc<Mutex<WorldAccessDestructor>>>()
                .unwrap()
                .clone(),
        ))
    }
}

impl ObjectRef {
    pub fn new(lua: &Lua, e: Entity) -> ObjectRef {
        unsafe {
            let wa = WorldAccess::fetch_readonly(lua);
            wa.access_commands().entity(e).inc_ref()
        };
        ObjectRef(
            e,
            lua.weak(),
            lua.app_data_ref::<Arc<Mutex<WorldAccessDestructor>>>()
                .unwrap()
                .clone(),
        )
    }
    pub unsafe fn new_no_inc_ref(lua: &Lua, e: Entity) -> ObjectRef {
        ObjectRef(
            e,
            lua.weak(),
            lua.app_data_ref::<Arc<Mutex<WorldAccessDestructor>>>()
                .unwrap()
                .clone(),
        )
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
            1: lua.weak(),
            2: self.2.clone(),
        }
    }
    pub unsafe fn clone_no_inc_ref(&self) -> Self {
        Self {
            0: self.0,
            1: self.1.clone(),
            2: self.2.clone(),
        }
    }
    pub fn change_lua(mut self, lua: &Lua) -> Self {
        self.1 = lua.weak();
        self
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
        if let Some(lua) = self.1.try_upgrade() {
            unsafe {
                WorldAccess::fetch_readonly(&lua)
                    .access_commands()
                    .entity(self.0)
                    .dec_ref()
            };
        } else {
            match &*self.2.lock() {
                WorldAccessDestructor::None => unreachable!(
                    "Luau container is being destroyed, but not by the world access collector. Failed to lock onto a valid world access destructor."
                ),
                WorldAccessDestructor::DestructPhase { commands } => {
                    commands
                        .lock()
                        .push(EntityCommandWrapper::new(dec_ref_command, self.0));
                }
            }
        }
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
        methods.add_meta_method(
            "__eq",
            move |_, t, (o,): (LuaUserDataRef<ObjectRef>,)| -> LuaResult<bool> { Ok(t.0 == o.0) },
        )
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
        Self(self.0.clone(), self.1.clone(), self.2.clone())
    }
}
