use crate::{
    enums::LuaEnums,
    userdata::{
        CFrame, LuaSendRaycastParams, ObjectRef, Ray, RaycastParams, RaycastResult, Vector2,
        Vector2int16, Vector3, Vector3int16,
    },
};

use bevy::{math::Vec3, reflect::Reflect};
use mlua::{
    ffi::{lua_Integer, lua_Number},
    prelude::*,
};

#[derive(Clone, Default, Debug, Reflect, PartialEq)]
#[non_exhaustive]
pub enum LuaFreeValue {
    #[default]
    Nil,
    Boolean(bool),
    Integer(lua_Integer),
    Number(lua_Number),
    Vector(Vec3),
    String(String),
    Buffer(Vec<u8>),

    Object(ObjectRef),

    CFrame(CFrame),
    Vector3(Vector3),
    Vector2(Vector2),
    Vector3int16(Vector3int16),
    Vector2int16(Vector2int16),

    EnumItem(String, String),
    Enum(String),
    Enums,

    Ray(Ray),
    RaycastParams(LuaSendRaycastParams),
    RaycastResult(RaycastResult),
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
                        any_user_data.borrow::<ObjectRef>()?.clone(),
                    )),
                    "CFrame" => Ok(LuaFreeValue::CFrame(*any_user_data.borrow::<CFrame>()?)),
                    "Vector3" => Ok(LuaFreeValue::Vector3(*any_user_data.borrow::<Vector3>()?)),
                    "Vector2" => Ok(LuaFreeValue::Vector2(*any_user_data.borrow::<Vector2>()?)),
                    "Vector3int16" => Ok(LuaFreeValue::Vector3int16(
                        *any_user_data.borrow::<Vector3int16>()?,
                    )),
                    "Vector2int16" => Ok(LuaFreeValue::Vector2int16(
                        *any_user_data.borrow::<Vector2int16>()?,
                    )),
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
                    "Ray" => Ok(LuaFreeValue::Ray(*any_user_data.borrow::<Ray>()?)),
                    "RaycastParams" => Ok(LuaFreeValue::RaycastParams(
                        any_user_data.borrow::<RaycastParams>()?.as_send()?,
                    )),
                    "RaycastResult" => Ok(LuaFreeValue::RaycastResult(
                        any_user_data.borrow::<RaycastResult>()?.clone_lua(lua),
                    )),
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
            LuaFreeValue::Object(o) => o.into_lua(lua),
            LuaFreeValue::Buffer(items) => Ok(LuaValue::Buffer(lua.create_buffer(items)?)),
            LuaFreeValue::CFrame(cframe) => cframe.into_lua(lua),
            LuaFreeValue::Vector3(vector3) => vector3.into_lua(lua),
            LuaFreeValue::Vector2(vector2) => vector2.into_lua(lua),
            LuaFreeValue::Vector3int16(vector3int16) => vector3int16.into_lua(lua),
            LuaFreeValue::Vector2int16(vector2int16) => vector2int16.into_lua(lua),
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
            LuaFreeValue::Ray(ray) => ray.into_lua(lua),
            LuaFreeValue::RaycastParams(lua_send_raycast_params) => {
                lua_send_raycast_params.into_lua(lua)
            }
            LuaFreeValue::RaycastResult(raycast_result) => {
                raycast_result.clone_lua(lua).into_lua(lua)
            }
        }
    }
}

pub trait LuaValueExt: Sized {
    fn into_free_value(self, lua: &Lua) -> LuaResult<LuaFreeValue>;
}

impl LuaValueExt for LuaValue {
    fn into_free_value(self, lua: &Lua) -> LuaResult<LuaFreeValue> {
        LuaFreeValue::from_lua(self, lua)
    }
}

#[diagnostic::on_unimplemented(
    message = "{Self} is not transferrable across the Luau VM boundary",
    label = "cannot be sent between Luau VMs",
    note = "you could probably convert this into a LuaFreeValue to transfer it"
)]
pub auto trait LuaSend {}

impl !LuaSend for LuaValue {}
impl !LuaSend for LuaMultiValue {}
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
