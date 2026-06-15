use std::mem::replace;

use bevy::prelude::{Deref, DerefMut};
use mlua::{FromLua, IntoLua};

#[repr(transparent)]
#[derive(PartialEq, Eq, Debug, Hash, Deref, DerefMut, Default, PartialOrd, Ord)]
pub struct EraseOnClone<T>(#[deref] pub T);

impl<T: FromLua> FromLua for EraseOnClone<T> {
    fn from_lua(
        value: mlua::prelude::LuaValue,
        lua: &mlua::prelude::Lua,
    ) -> mlua::prelude::LuaResult<Self> {
        Ok(EraseOnClone(T::from_lua(value, lua)?))
    }
}
impl<T: IntoLua> IntoLua for EraseOnClone<T> {
    fn into_lua(
        self,
        lua: &mlua::prelude::Lua,
    ) -> mlua::prelude::LuaResult<mlua::prelude::LuaValue> {
        self.0.into_lua(lua)
    }
}

impl<T> Clone for EraseOnClone<T>
where
    EraseOnClone<T>: Default,
{
    fn clone(&self) -> Self {
        Self::default()
    }
}
impl<T> EraseOnClone<T>
where
    EraseOnClone<T>: Default,
{
    pub fn take(&mut self) -> Self {
        replace(self, Self::default())
    }
}
