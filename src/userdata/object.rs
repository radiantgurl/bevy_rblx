use std::ops::Deref;

use crate::{
    core::{WorldAccess, bevy::RefCountedComponent, object::ObjectHeader},
    internal_prelude::*,
};
use bevy::prelude::*;
use mlua::prelude::*;

use bevy::ecs::entity::Entity;
#[derive(Reflect, Debug)]
pub struct ObjectRef {
    e: Entity,
    reference: RefCountedComponent,
}
impl PartialEq for ObjectRef {
    fn eq(&self, other: &Self) -> bool {
        self.e == other.e
    }
}

impl FromLua for ObjectRef {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        let v: LuaUserDataRef<Self> = value.borrow_typed()?;
        unsafe {
            v.reference.inc();
        }
        Ok(Self {
            e: v.e,
            reference: v.reference.reference(),
        })
    }
}

impl ObjectRef {
    pub fn new(lua: &Lua, e: Entity) -> ObjectRef {
        let reference = unsafe {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            let refcounted = world.get::<RefCountedComponent>(e).unwrap();
            refcounted.inc();
            refcounted.reference()
        };
        ObjectRef { e, reference }
    }
    pub fn new_world(world: &World, e: Entity) -> ObjectRef {
        let reference = world.get::<RefCountedComponent>(e).unwrap().reference();
        unsafe {
            reference.inc();
        }
        ObjectRef { e, reference }
    }
    pub fn entity(&self) -> Entity {
        self.e
    }
}

impl Deref for ObjectRef {
    type Target = Entity;

    fn deref(&self) -> &Self::Target {
        &self.e
    }
}

impl Drop for ObjectRef {
    fn drop(&mut self) {
        unsafe {
            self.reference.dec();
        }
    }
}

impl LuaUserData for ObjectRef {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", move |l, t, ()| -> LuaResult<String> {
            let world_access = WorldAccess::fetch_readonly(l);
            let world = world_access.access_read_only();
            let string = world
                .get::<Name>(t.e)
                .map(|x| x.as_str())
                .unwrap_or_else(|| world.get::<ObjectHeader>(t.e).unwrap().vtable.class_name)
                .to_owned();
            Ok(string)
        });
        methods.add_meta_method(
            "__index",
            move |l, t, (k,): (String,)| -> LuaResult<LuaValue> {
                let vtable = WorldAccess::fetch_readonly(l)
                    .access_read_only()
                    .get::<ObjectHeader>(t.e)
                    .unwrap()
                    .vtable;
                vtable.get(l, t.e, k)
            },
        );
        methods.add_meta_method(
            "__newindex",
            move |l, t, (k, v): (String, LuaValue)| -> LuaResult<()> {
                let vtable = WorldAccess::fetch_readonly(l)
                    .access_read_only()
                    .get::<ObjectHeader>(t.e)
                    .unwrap()
                    .vtable;
                vtable.set(l, t.e, k, v)
            },
        );
        methods.add_meta_method(
            "__eq",
            move |_, t, (o,): (LuaUserDataRef<ObjectRef>,)| -> LuaResult<bool> { Ok(t.e == o.e) },
        )
    }
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "Object")
    }
}

impl Clone for ObjectRef {
    fn clone(&self) -> Self {
        unsafe {
            self.reference.inc();
        }
        Self {
            e: self.e,
            reference: self.reference.reference(),
        }
    }
}
