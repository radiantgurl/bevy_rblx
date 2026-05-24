use bevy::platform::collections::HashMap;
use bevy_rblx_derive::register;
use mlua::prelude::*;

use crate::core::lua::singleton::LuaSingleton;
use crate::internal_prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct CachedLuaFunction(fn(&Lua) -> LuaFunction);

impl CachedLuaFunction {
    pub fn fetch(&self, lua: &Lua) -> LuaFunction {
        lua.app_data_mut::<CachedLuaFunctions>()
            .unwrap()
            .0
            .entry(self.0 as usize)
            .or_insert_with(|| self.0(lua))
            .clone()
    }
    pub const fn new(generator: fn(&Lua) -> LuaFunction) -> Self {
        Self(generator)
    }
}

#[derive(Default)]
struct CachedLuaFunctions(HashMap<usize, LuaFunction>);

#[register]
impl LuaSingleton for CachedLuaFunctions {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.set_app_data(CachedLuaFunctions::default());
        Ok(())
    }
}
