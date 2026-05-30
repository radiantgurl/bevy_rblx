use crate::{
    enums::LuaEnums,
    userdata::{CFrame, ObjectRef, Vector2, Vector3},
};

use bevy::{math::Vec3, reflect::Reflect};
use mlua::{
    ffi::{lua_Integer, lua_Number},
    prelude::*,
};

#[derive(Clone, Default, Debug, Reflect)]
#[non_exhaustive]
pub enum LuaFreeValue {
    #[default]
    Nil,
    Boolean(bool),
    Integer(lua_Integer),
    Number(lua_Number),
    Vector(Vec3),
    String(String),
    Object(ObjectRef),
    CFrame(CFrame),
    Vector3(Vector3),
    Vector2(Vector2),
    Buffer(Vec<u8>),
    EnumItem(String, String),
    Enum(String),
    Enums,
}

impl FromLua for LuaFreeValue {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaNil => Ok(Self::Nil),
            LuaValue::Boolean(b) => Ok(LuaFreeValue::Boolean(b)),
            LuaValue::LightUserData(_) => todo!(),
            LuaValue::Integer(i) => Ok(LuaFreeValue::Integer(i)),
            LuaValue::Number(n) => Ok(LuaFreeValue::Number(n)),
            LuaValue::Vector(vector) => Ok(LuaFreeValue::Vector(Vec3 {
                x: vector.x(),
                y: vector.y(),
                z: vector.z(),
            })),
            LuaValue::String(s) => Ok(LuaFreeValue::String(s.to_string_lossy())),
            LuaValue::Table(table) => todo!(),
            LuaValue::Function(_) => Err(LuaError::runtime(
                "cannot convert function to lua free value",
            )),
            LuaValue::Thread(_) => {
                Err(LuaError::runtime("cannot convert thread to lua free value"))
            }
            LuaValue::UserData(any_user_data) => {
                let type_name = any_user_data.type_name()?.ok_or_else(|| {
                    LuaError::runtime("serializing to free value not implemented for unknown type")
                })?;
                match type_name.as_str() {
                    "Instance" | "Object" => Ok(LuaFreeValue::Object(
                        any_user_data.borrow::<ObjectRef>()?.clone_lua(lua),
                    )),
                    "CFrame" => Ok(LuaFreeValue::CFrame(*any_user_data.borrow::<CFrame>()?)),
                    "Vector3" => Ok(LuaFreeValue::Vector3(*any_user_data.borrow::<Vector3>()?)),
                    "Vector2" => Ok(LuaFreeValue::Vector2(*any_user_data.borrow::<Vector2>()?)),
                    "EnumItem" => {
                        let origin = any_user_data.get::<String>("Origin")?;
                        let value = any_user_data.get::<String>("Name")?;
                        Ok(LuaFreeValue::EnumItem(origin, value))
                    }
                    "Enum" => {
                        let origin = any_user_data.get::<String>("ENUM_NAME")?;
                        Ok(LuaFreeValue::Enum(origin))
                    }
                    "Enums" => Ok(LuaFreeValue::Enums),
                    _ => todo!(
                        "serializing to free value not implemented for userdata type {type_name}"
                    ),
                }
            }
            LuaValue::Buffer(buffer) => Ok(LuaFreeValue::Buffer(buffer.to_vec())),
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
            LuaFreeValue::Vector(v) => Ok(LuaValue::Vector(LuaVector::new(v.x, v.y, v.z))),
            LuaFreeValue::String(s) => s.into_lua(lua),
            LuaFreeValue::Object(o) => o.change_lua(lua).into_lua(lua),
            LuaFreeValue::Buffer(items) => Ok(LuaValue::Buffer(lua.create_buffer(items)?)),
            LuaFreeValue::CFrame(cframe) => cframe.into_lua(lua),
            LuaFreeValue::Vector3(vector3) => vector3.into_lua(lua),
            LuaFreeValue::Vector2(vector2) => vector2.into_lua(lua),
            LuaFreeValue::EnumItem(origin, value) => {
                let enums = LuaEnums.into_lua(lua)?;
                let enums_ud = enums.as_userdata().unwrap();
                enums_ud.get_path::<LuaValue>(format!("{origin}.{value}").as_str())
            }
            LuaFreeValue::Enum(origin) => {
                let enums = LuaEnums.into_lua(lua)?;
                let enums_ud = enums.as_userdata().unwrap();
                enums_ud.get::<LuaValue>(origin.as_str())
            }
            LuaFreeValue::Enums => LuaEnums.into_lua(lua),
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
            LuaFreeValue::Vector(v) => Ok(LuaValue::Vector(LuaVector::new(v.x, v.y, v.z))),
            LuaFreeValue::String(s) => s.as_str().into_lua(lua),
            LuaFreeValue::Object(o) => o.clone_lua(lua).into_lua(lua),
            LuaFreeValue::Buffer(items) => Ok(LuaValue::Buffer(lua.create_buffer(items)?)),
            LuaFreeValue::CFrame(cframe) => (*cframe).into_lua(lua),
            LuaFreeValue::Vector3(vector3) => (*vector3).into_lua(lua),
            LuaFreeValue::Vector2(vector2) => (*vector2).into_lua(lua),
            LuaFreeValue::EnumItem(origin, value) => {
                let enums = LuaEnums.into_lua(lua)?;
                let enums_ud = enums.as_userdata().unwrap();
                enums_ud.get_path::<LuaValue>(format!("{origin}.{value}").as_str())
            }
            LuaFreeValue::Enum(origin) => {
                let enums = LuaEnums.into_lua(lua)?;
                let enums_ud = enums.as_userdata().unwrap();
                enums_ud.get::<LuaValue>(origin.as_str())
            }
            LuaFreeValue::Enums => LuaEnums.into_lua(lua),
        }
    }
}

#[diagnostic::on_unimplemented(
    message = "{Self} is not transferrable across Lua instances",
    label = "cannot be shared between Lua instances",
    note = "you could probably convert this into a LuaFreeValue to transfer it between instances if the type is primitive enough"
)]
pub auto trait LuaSend {}

impl !LuaSend for LuaValue {}
impl !LuaSend for LuaTable {}
impl !LuaSend for LuaFunction {}
impl !LuaSend for LuaThread {}
impl !LuaSend for LuaAnyUserData {}
impl LuaSend for WeakLua {}
impl LuaSend for Lua {}

#[cfg(test)]
mod tests {
    use static_assertions::assert_impl_all;

    use crate::userdata::{LuaFreeValue, LuaSend};

    assert_impl_all!(LuaFreeValue: LuaSend);
}
