use std::{
    mem::transmute,
    ops::{Deref, DerefMut},
};

use crate as bevy_rblx;
use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::{AppDataRefMut, prelude::*};

use crate::core::LuaSingleton;

#[derive(Default, Debug)]
pub struct WorldAccess(Option<&'static mut World>);
impl LuaUserData for WorldAccess {}

#[register]
impl LuaSingleton for WorldAccess {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.set_app_data(Self(None));
        Ok(())
    }
}

impl WorldAccess {
    pub fn fetch<'a>(lua: &'a Lua) -> AppDataRefMut<'a, WorldAccess> {
        lua.app_data_mut().unwrap()
    }
    pub unsafe fn insert(&mut self, w: &mut World) {
        self.0 = Some(unsafe { transmute::<_, &'static mut World>(w) });
    }
    pub fn clear(&mut self) {
        self.0 = None;
    }
}

impl Deref for WorldAccess {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("Failed to access current World state!\nThis can occur by calling into Lua without first setting the world guard.\nCheck backtrace to see where this happened :)")
    }
}

impl DerefMut for WorldAccess {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_deref_mut().expect("Failed to access current World state!\nThis can occur by calling into Lua without first setting the world guard.\nCheck backtrace to see where this happened :)")
    }
}
