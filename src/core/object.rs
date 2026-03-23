use bevy::prelude::*;
use mlua::prelude::*;

#[derive(Component, Clone)]
pub struct ObjectHeader(pub &'static ObjectVTable);

impl std::fmt::Debug for ObjectHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ObjectHeader").field(self.0).finish()
    }
}

#[derive(Debug)]
pub enum ObjectNewFn {
    None,
    Protected(fn(&Lua, EntityCommands) -> LuaResult<()>),
    Visible(fn(&Lua, EntityCommands) -> LuaResult<()>),
}

#[derive(Debug)]
pub struct ObjectVTable {
    pub class_name: &'static str,
    pub hierarchy: &'static [&'static str],

    pub index: fn(&Lua, Entity, String) -> LuaResult<LuaValue>,
    pub newindex: Option<fn(&Lua, Entity, String, LuaValue) -> LuaResult<()>>,
    pub tostring: Option<fn(&Lua, Entity) -> LuaResult<String>>,

    pub new: ObjectNewFn,
}

inventory::collect!(ObjectVTable);
