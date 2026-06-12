use std::mem::forget;

use mlua::prelude::*;

use crate::{
    core::WorldAccess,
    userdata::{LuaSend, RBXScriptSignal},
};

pub struct EventWithArgs<T: IntoLuaMulti + Clone + LuaSend + ?Sized> {
    event: RBXScriptSignal,
    values: T,
}

impl<T: IntoLuaMulti + Clone + LuaSend + ?Sized> Clone for EventWithArgs<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            event: self.event.reference(),
            values: self.values.clone(),
        }
    }
}

impl<T: IntoLuaMulti + Clone + LuaSend + ?Sized> EventWithArgs<T> {
    #[inline]
    pub const fn new(event: RBXScriptSignal, values: T) -> Self {
        Self { event, values }
    }
    #[inline]
    pub fn fire_in_lua(self, lua: &Lua) -> LuaResult<()> {
        let EventWithArgs { event, values } = self;
        event.fire_in_lua(lua, false, values)
    }
    #[inline]
    pub fn fire_outside_lua(self, wa: &mut WorldAccess) -> LuaResult<()> {
        let EventWithArgs { event, values } = self;
        event.fire_outside_lua(wa, false, values)
    }
    #[inline]
    pub fn deconstruct(self) -> (RBXScriptSignal, T) {
        let EventWithArgs { event, values } = self;
        (event, values)
    }
}

#[derive(Default)]
struct QueueNoDrop;

impl Drop for QueueNoDrop {
    #[inline]
    fn drop(&mut self) {
        panic!("expected queue to be fired before being consumed");
    }
}

pub struct EventQueue<T: IntoLuaMulti + Clone + LuaSend> {
    queued: Vec<EventWithArgs<T>>,
    no_drop: QueueNoDrop,
}

impl<T: IntoLuaMulti + Clone + LuaSend> EventQueue<T> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            queued: Vec::new(),
            no_drop: QueueNoDrop,
        }
    }
    #[inline]
    pub fn queue(&mut self, event: RBXScriptSignal, values: T) {
        self.queued.push(EventWithArgs::new(event, values));
    }
    #[inline]
    pub fn fire_in_lua(self, lua: &Lua) -> LuaResult<()> {
        let EventQueue { queued, no_drop } = self;
        forget(no_drop);
        for e in queued {
            e.fire_in_lua(lua)?;
        }
        Ok(())
    }
    #[inline]
    pub fn fire_outside_lua(self, wa: &mut WorldAccess) -> LuaResult<()> {
        let EventQueue { queued, no_drop } = self;
        forget(no_drop);
        for e in queued {
            e.fire_outside_lua(wa)?;
        }
        Ok(())
    }
}

#[must_use]
#[derive(Default)]
pub struct EventQueueNoArgs {
    queued: Vec<RBXScriptSignal>,
}

impl EventQueueNoArgs {
    #[inline]
    pub const fn new() -> Self {
        Self { queued: Vec::new() }
    }
    #[inline]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            queued: Vec::with_capacity(cap),
        }
    }
    #[inline]
    pub fn queue(&mut self, event: RBXScriptSignal) {
        self.queued.push(event);
    }
    #[inline]
    pub fn fire_in_lua(
        &self,
        lua: &Lua,
        values: impl IntoLuaMulti + LuaSend + Clone,
    ) -> LuaResult<()> {
        for e in &self.queued {
            e.fire_in_lua(lua, false, values.clone())?;
        }
        Ok(())
    }
    #[inline]
    pub fn fire_outside_lua(
        &self,
        wa: &mut WorldAccess,
        values: impl IntoLuaMulti + LuaSend + Clone,
    ) -> LuaResult<()> {
        for e in &self.queued {
            e.fire_outside_lua(wa, false, values.clone())?;
        }
        Ok(())
    }
}
