use crate::{
    core::LuaSingleton,
    enums::{Axis, NormalId},
    internal_prelude::*,
};
use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::prelude::*;

#[derive(Clone, Copy, FromLua, Debug, PartialEq, Default)]
#[repr(transparent)]
pub struct Vector3(Vec3);

impl From<Vec3> for Vector3 {
    fn from(value: Vec3) -> Self {
        Self(value)
    }
}
impl Into<Vec3> for Vector3 {
    fn into(self) -> Vec3 {
        self.0
    }
}

impl LuaUserData for Vector3 {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("X", |_lua, this| Ok(this.0.x));
        fields.add_field_method_get("Y", |_lua, this| Ok(this.0.y));
        fields.add_field_method_get("Z", |_lua, this| Ok(this.0.z));
        fields.add_field_method_get("Magnitude", |_lua, this| Ok(this.0.length()));
        fields.add_field_method_get("Unit", |_lua, this| Ok(Self(this.0.normalize_or_zero())));

        fields.add_meta_field("__type", "Vector3");
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Abs", |_lua, this, ()| Ok(Self(this.0.abs())));
        methods.add_method("Ceil", |_lua, this, ()| Ok(Self(this.0.ceil())));
        methods.add_method("Floor", |_lua, this, ()| Ok(Self(this.0.floor())));
        methods.add_method("Sign", |_lua, this, ()| Ok(Self(this.0.signum())));
        methods.add_method("Cross", |_lua, this, (o,): (Vector3,)| {
            Ok(Self(this.0.cross(o.0)))
        });
        methods.add_method("Angle", |_lua, this, (o, a): (Vector3, Option<Axis>)| {
            if let Some(_a) = a {
                lua_todo!("Angle between on specific axes are not implemented yet")
            } else {
                Ok(this.0.angle_between(o.0))
            }
        });
        methods.add_method("Dot", |_lua, this, (o,): (Vector3,)| Ok(this.0.dot(o.0)));
        methods.add_method("FuzzyEq", |_lua, this, (o, e): (Vector3, f64)| {
            Ok(this.0.abs_diff_eq(o.0, e as f32))
        });
        methods.add_method("Lerp", |_lua, this, (o, e): (Vector3, f64)| {
            Ok(Self(this.0.lerp(o.0, e as f32)))
        });
        methods.add_method("Max", |_lua, this, (o,): (Vector3,)| {
            Ok(Self(this.0.max(o.0)))
        });
        methods.add_method("Min", |_lua, this, (o,): (Vector3,)| {
            Ok(Self(this.0.min(o.0)))
        });
        methods.add_meta_method("__add", |_lua, this, (o,): (Vector3,)| {
            Ok(Self(this.0 + o.0))
        });
        methods.add_meta_method("__sub", |_lua, this, (o,): (Vector3,)| {
            Ok(Self(this.0 - o.0))
        });
        methods.add_meta_method("__mul", |lua, this, (o,): (LuaValue,)| {
            if let Some(ud) = o.as_userdata() {
                let x: LuaUserDataRef<Self> = ud.borrow_typed()?;
                Ok(Self(this.0 * x.0))
            } else {
                Ok(Self(this.0 * f64::from_lua(o, lua)? as f32))
            }
        });
        methods.add_meta_method("__div", |lua, this, (o,): (LuaValue,)| {
            if let Some(ud) = o.as_userdata() {
                let x: LuaUserDataRef<Self> = ud.borrow_typed()?;
                Ok(Self(this.0 / x.0))
            } else {
                Ok(Self(this.0 / f64::from_lua(o, lua)? as f32))
            }
        });
        methods.add_meta_method("__idiv", |lua, this, (o,): (LuaValue,)| {
            if let Some(ud) = o.as_userdata() {
                let x: LuaUserDataRef<Self> = ud.borrow_typed()?;
                Ok(Self((this.0 / x.0).floor()))
            } else {
                Ok(Self((this.0 / f64::from_lua(o, lua)? as f32).floor()))
            }
        });
    }
}

impl Vector3 {
    pub fn new(_lua: &Lua, (x, y, z): (f64, f64, f64)) -> LuaResult<Vector3> {
        Ok(Vector3(Vec3 {
            x: x as f32,
            y: y as f32,
            z: z as f32,
        }))
    }
    pub fn from_normal_id(_lua: &Lua, normal_id: NormalId) -> LuaResult<Vector3> {
        Ok(Self(match normal_id {
            NormalId::Right => Vec3::new(1f32, 0f32, 0f32),
            NormalId::Top => Vec3::new(0f32, 1f32, 0f32),
            NormalId::Back => Vec3::new(0f32, 0f32, 1f32),
            NormalId::Left => Vec3::new(-1f32, 0f32, 0f32),
            NormalId::Bottom => Vec3::new(0f32, -1f32, 0f32),
            NormalId::Front => Vec3::new(0f32, 0f32, -1f32),
        }))
    }
    pub fn from_axis(_lua: &Lua, axis: Axis) -> LuaResult<Vector3> {
        Ok(Self(match axis {
            Axis::X => Vec3::new(1f32, 0f32, 0f32),
            Axis::Y => Vec3::new(0f32, 1f32, 0f32),
            Axis::Z => Vec3::new(0f32, 0f32, 1f32),
        }))
    }
}

#[register]
impl LuaSingleton for Vector3 {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        let table = lua.create_table()?;
        table.raw_set("zero", Vector3(Vec3::ZERO))?;
        table.raw_set("one", Vector3(Vec3::ONE))?;
        table.raw_set("xAxis", Vector3(Vec3::X))?;
        table.raw_set("yAxis", Vector3(Vec3::Y))?;
        table.raw_set("zAxis", Vector3(Vec3::Z))?;
        table.raw_set("new", lua.create_function(Vector3::new)?)?;
        table.raw_set(
            "FromNormalid",
            lua.create_function(Vector3::from_normal_id)?,
        )?;
        table.raw_set("FromAxis", lua.create_function(Vector3::from_axis)?)?;
        table.set_readonly(true);
        lua.globals().raw_set("Vector3", table)?;
        Ok(())
    }
}
