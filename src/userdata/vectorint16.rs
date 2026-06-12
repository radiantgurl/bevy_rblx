use std::ops::{Add, Div, Mul, Sub};

use crate::internal_prelude::*;
use bevy::reflect::Reflect;
use bevy_rblx_derive::register;
use mlua::prelude::*;

use crate::core::lua::LuaSingleton;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug, FromLua, Reflect)]
pub struct Vector2int16 {
    pub x: i16,
    pub y: i16,
}

impl Add for Vector2int16 {
    type Output = Vector2int16;

    fn add(self, rhs: Self) -> Self::Output {
        Vector2int16 {
            x: self.x.wrapping_add(rhs.x),
            y: self.y.wrapping_add(rhs.y),
        }
    }
}
impl Sub for Vector2int16 {
    type Output = Vector2int16;

    fn sub(self, rhs: Self) -> Self::Output {
        Vector2int16 {
            x: self.x.wrapping_sub(rhs.x),
            y: self.y.wrapping_sub(rhs.y),
        }
    }
}
impl Mul for Vector2int16 {
    type Output = Vector2int16;

    fn mul(self, rhs: Self) -> Self::Output {
        Vector2int16 {
            x: self.x.wrapping_mul(rhs.x),
            y: self.y.wrapping_mul(rhs.y),
        }
    }
}
impl Mul<i16> for Vector2int16 {
    type Output = Vector2int16;

    fn mul(self, rhs: i16) -> Self::Output {
        Vector2int16 {
            x: self.x.wrapping_mul(rhs),
            y: self.y.wrapping_mul(rhs),
        }
    }
}

impl Div for Vector2int16 {
    type Output = Vector2int16;

    fn div(self, rhs: Self) -> Self::Output {
        Vector2int16 {
            x: self.x.wrapping_div(rhs.x),
            y: self.y.wrapping_div(rhs.y),
        }
    }
}
impl Div<i16> for Vector2int16 {
    type Output = Vector2int16;

    fn div(self, rhs: i16) -> Self::Output {
        Vector2int16 {
            x: self.x.wrapping_div(rhs),
            y: self.y.wrapping_div(rhs),
        }
    }
}

impl LuaUserData for Vector2int16 {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("X", |_, this| Ok(this.x));
        fields.add_field_method_get("Y", |_, this| Ok(this.y));

        fields.add_meta_field("__type", "Vector2int16");
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__add", |_, this, o: Vector2int16| Ok(*this + o));
        methods.add_meta_method("__sub", |_, this, o: Vector2int16| Ok(*this - o));
        methods.add_meta_method("__mul", |lua, this, v: LuaValue| {
            if v.is_number() {
                Ok(this.mul(i16::from_lua(v, lua)?))
            } else {
                Ok(this.mul(Vector2int16::from_lua(v, lua)?))
            }
        });
        methods.add_meta_method("__div", |lua, this, v: LuaValue| {
            if v.is_number() {
                Ok(this.div(i16::from_lua(v, lua)?))
            } else {
                Ok(this.div(Vector2int16::from_lua(v, lua)?))
            }
        });

        methods.add_meta_method("__tostring", |_, this, (): ()| {
            Ok(format!("({}, {})", this.x, this.y))
        });
    }
}

#[register]
impl LuaSingleton for Vector2int16 {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        let table = lua.create_table()?;

        table.raw_set(
            "new",
            lua.create_function(|_, (x, y): (i16, i16)| Ok(Vector2int16 { x, y }))?,
        )?;
        table.set_readonly(true);

        lua.globals().raw_set("Vector2int16", table)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug, FromLua, Reflect)]
pub struct Vector3int16 {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

impl Add for Vector3int16 {
    type Output = Vector3int16;

    fn add(self, rhs: Self) -> Self::Output {
        Vector3int16 {
            x: self.x.wrapping_add(rhs.x),
            y: self.y.wrapping_add(rhs.y),
            z: self.z.wrapping_add(rhs.z),
        }
    }
}
impl Sub for Vector3int16 {
    type Output = Vector3int16;

    fn sub(self, rhs: Self) -> Self::Output {
        Vector3int16 {
            x: self.x.wrapping_sub(rhs.x),
            y: self.y.wrapping_add(rhs.y),
            z: self.z.wrapping_sub(rhs.z),
        }
    }
}
impl Mul for Vector3int16 {
    type Output = Vector3int16;

    fn mul(self, rhs: Self) -> Self::Output {
        Vector3int16 {
            x: self.x.wrapping_mul(rhs.x),
            y: self.y.wrapping_mul(rhs.y),
            z: self.z.wrapping_mul(rhs.z),
        }
    }
}
impl Mul<i16> for Vector3int16 {
    type Output = Vector3int16;

    fn mul(self, rhs: i16) -> Self::Output {
        Vector3int16 {
            x: self.x.wrapping_mul(rhs),
            y: self.y.wrapping_mul(rhs),
            z: self.z.wrapping_mul(rhs),
        }
    }
}

impl Div for Vector3int16 {
    type Output = Vector3int16;

    fn div(self, rhs: Self) -> Self::Output {
        Vector3int16 {
            x: self.x.wrapping_div(rhs.x),
            y: self.y.wrapping_div(rhs.y),
            z: self.z.wrapping_div(rhs.z),
        }
    }
}
impl Div<i16> for Vector3int16 {
    type Output = Vector3int16;

    fn div(self, rhs: i16) -> Self::Output {
        Vector3int16 {
            x: self.x.wrapping_div(rhs),
            y: self.y.wrapping_div(rhs),
            z: self.z.wrapping_div(rhs),
        }
    }
}

impl LuaUserData for Vector3int16 {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("X", |_, this| Ok(this.x));
        fields.add_field_method_get("Y", |_, this| Ok(this.y));
        fields.add_field_method_get("Z", |_, this| Ok(this.z));

        fields.add_meta_field("__type", "Vector3int16");
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__add", |_, this, o: Vector3int16| Ok(*this + o));
        methods.add_meta_method("__sub", |_, this, o: Vector3int16| Ok(*this - o));
        methods.add_meta_method("__mul", |lua, this, v: LuaValue| {
            if v.is_number() {
                Ok(this.mul(i16::from_lua(v, lua)?))
            } else {
                Ok(this.mul(Vector3int16::from_lua(v, lua)?))
            }
        });
        methods.add_meta_method("__div", |lua, this, v: LuaValue| {
            if v.is_number() {
                Ok(this.div(i16::from_lua(v, lua)?))
            } else {
                Ok(this.div(Vector3int16::from_lua(v, lua)?))
            }
        });

        methods.add_meta_method("__tostring", |_, this, (): ()| {
            Ok(format!("({}, {}, {})", this.x, this.y, this.z))
        });
    }
}

#[register]
impl LuaSingleton for Vector3int16 {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        let table = lua.create_table()?;

        table.raw_set(
            "new",
            lua.create_function(|_, (x, y, z): (i16, i16, i16)| Ok(Vector3int16 { x, y, z }))?,
        )?;
        table.set_readonly(true);

        lua.globals().raw_set("Vector3int16", table)
    }
}
