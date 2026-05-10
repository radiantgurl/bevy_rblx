use crate::userdata::ObjectRef;

use bevy::{ecs::entity::Entity};
use mlua::{
    ffi::{lua_Integer, lua_Number},
    prelude::*,
};

#[derive(Clone, Default, Debug)]
#[non_exhaustive]
pub enum LuaFreeValue {
    #[default]
    Nil,
    Boolean(bool),
    Integer(lua_Integer),
    Number(lua_Number),
    Vector(LuaVector),
    String(String),
    Object(Entity),
    Buffer(Box<[u8]>),
}

impl FromLua for LuaFreeValue {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value { 
            LuaNil => Ok(Self::Nil),
            LuaValue::Boolean(b) => Ok(LuaFreeValue::Boolean(b)),
            LuaValue::LightUserData(_) => todo!(),
            LuaValue::Integer(i) => Ok(LuaFreeValue::Integer(i)),
            LuaValue::Number(n) => Ok(LuaFreeValue::Number(n)),
            LuaValue::Vector(vector) => Ok(LuaFreeValue::Vector(vector)),
            LuaValue::String(s) => Ok(LuaFreeValue::String(s.to_string_lossy())),
            LuaValue::Table(table) => todo!(),
            LuaValue::Function(_) => Err(LuaError::runtime(
                "cannot convert function to lua free value",
            )),
            LuaValue::Thread(_) => {
                Err(LuaError::runtime("cannot convert thread to lua free value"))
            }
            LuaValue::UserData(any_user_data) => todo!(),
            LuaValue::Buffer(buffer) => Ok(LuaFreeValue::Buffer(unsafe {
                let buf_vec = buffer.to_vec();
                let mut b = Box::new_uninit_slice(buf_vec.len()).assume_init();
                b.copy_from_slice(buf_vec.as_slice());
                b
            })),
            LuaValue::Error(e) => Err(e.into_lua_err()),
            LuaValue::Other(_) => unimplemented!(),
        }
    }
}

impl IntoLua for LuaFreeValue {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            LuaFreeValue::Nil => Ok(LuaValue::Nil),
            LuaFreeValue::Boolean(b) => Ok(LuaValue::Boolean(b)),
            LuaFreeValue::Integer(i) => Ok(LuaValue::Integer(i)),
            LuaFreeValue::Number(n) => Ok(LuaValue::Number(n)),
            LuaFreeValue::Vector(vector) => Ok(LuaValue::Vector(vector)),
            LuaFreeValue::String(s) => s.into_lua(lua),
            LuaFreeValue::Object(entity) => ObjectRef::new(lua, entity).into_lua(lua),
            LuaFreeValue::Buffer(items) => Ok(LuaValue::Buffer(lua.create_buffer(items)?)),
        }
    }
}

impl IntoLua for &LuaFreeValue {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            LuaFreeValue::Nil => Ok(LuaValue::Nil),
            LuaFreeValue::Boolean(b) => Ok(LuaValue::Boolean(*b)),
            LuaFreeValue::Integer(i) => Ok(LuaValue::Integer(*i)),
            LuaFreeValue::Number(n) => Ok(LuaValue::Number(*n)),
            LuaFreeValue::Vector(vector) => Ok(LuaValue::Vector(*vector)),
            LuaFreeValue::String(s) => s.as_str().into_lua(lua),
            LuaFreeValue::Object(entity) => ObjectRef::new(lua, *entity).into_lua(lua),
            LuaFreeValue::Buffer(items) => Ok(LuaValue::Buffer(lua.create_buffer(items)?)),
        }
    }
}

#[diagnostic::on_unimplemented(
    message = "{Self} is not transferrable across Lua instances",
    label = "{Self} is local to a Lua instance",
    note = "you should use this with its own lua reference by storing a WeakLua reference"
)]
pub auto trait LuaSend {}

impl !LuaSend for LuaValue {}
impl !LuaSend for LuaTable {}
impl !LuaSend for LuaFunction {}
impl !LuaSend for LuaThread {}
impl !LuaSend for LuaAnyUserData {}
impl LuaSend for WeakLua {}
impl LuaSend for Lua {}