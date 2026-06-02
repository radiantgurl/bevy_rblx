use std::mem::replace;

use bevy::reflect::Reflect;
use mlua::prelude::*;

use crate::userdata::LuaSend;

#[derive(Clone, Reflect, Debug)]
#[reflect(opaque)]
pub enum DynamicLuaVec<T: FromLua + IntoLua + Clone> {
    Lua(LuaTable),
    Static(Vec<T>),
}

impl<T: FromLua + IntoLua + Clone> Default for DynamicLuaVec<T> {
    fn default() -> Self {
        Self::Static(Vec::default())
    }
}

impl<T: FromLua + IntoLua + Clone> DynamicLuaVec<T> {
    pub fn get_table(&mut self, lua: &Lua) -> LuaResult<LuaTable> {
        match self {
            Self::Lua(table) => return Ok(table.clone()),
            _ => (),
        };
        let table = lua.create_table()?;
        let items = match replace(self, Self::Lua(table.clone())) {
            DynamicLuaVec::Static(items) => items,
            _ => unreachable!(),
        };
        for (i, v) in items.into_iter().enumerate() {
            table.raw_set(i, v)?;
        }
        Ok(table)
    }
    pub fn as_static(&mut self) -> LuaResult<&mut Vec<T>> {
        let items = match self {
            Self::Static(v) => return Ok(v),
            Self::Lua(t) => Self::static_from_lua(t)?,
        };
        *self = Self::Static(items);
        match self {
            DynamicLuaVec::Static(items) => Ok(items),
            _ => unreachable!(),
        }
    }
    pub fn static_from_lua(table: &LuaTable) -> LuaResult<Vec<T>> {
        let mut v = Vec::new();
        for i in 1..=table.raw_len() {
            v.push(table.raw_get::<T>(i)?);
        }
        Ok(v)
    }

    pub fn static_into_lua(items: Vec<T>, lua: &Lua) -> LuaResult<LuaTable> {
        let table = lua.create_table()?;
        for (i, v) in items.into_iter().enumerate() {
            table.raw_set(i, v)?;
        }
        Ok(table)
    }
}

impl<T: FromLua + IntoLua + Clone> LuaSend for DynamicLuaVec<T> {}

impl<T: FromLua + IntoLua + Clone> IntoLua for &DynamicLuaVec<T> {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            DynamicLuaVec::Lua(table) => table.into_lua(lua),
            DynamicLuaVec::Static(items) => items.clone().into_lua(lua),
        }
    }
}
impl<T: FromLua + IntoLua + Clone> IntoLua for DynamicLuaVec<T> {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            DynamicLuaVec::Lua(table) => table.into_lua(lua),
            DynamicLuaVec::Static(items) => items.into_lua(lua),
        }
    }
}

impl<T: FromLua + IntoLua + Clone> FromLua for DynamicLuaVec<T> {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        Ok(Self::Lua(LuaTable::from_lua(value, lua)?))
    }
}
