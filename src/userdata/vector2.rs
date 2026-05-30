use crate::{core::lua::LuaSingleton, internal_prelude::*};
use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::prelude::*;

#[derive(Clone, Copy, FromLua, Debug, PartialEq, Default, Reflect)]
#[repr(transparent)]
pub struct Vector2(Vec2);

impl From<Vec2> for Vector2 {
    fn from(value: Vec2) -> Self {
        Self(value)
    }
}
impl Into<Vec2> for Vector2 {
    fn into(self) -> Vec2 {
        self.0
    }
}

impl LuaUserData for Vector2 {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("X", |_lua, this| Ok(this.0.x));
        fields.add_field_method_get("Y", |_lua, this| Ok(this.0.y));
        fields.add_field_method_get("Magnitude", |_lua, this| Ok(this.0.length()));
        fields.add_field_method_get("Unit", |_lua, this| Ok(Self(this.0.normalize_or_zero())));

        fields.add_meta_field("__type", "Vector2");
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Abs", |_lua, this, ()| Ok(Self(this.0.abs())));
        methods.add_method("Ceil", |_lua, this, ()| Ok(Self(this.0.ceil())));
        methods.add_method("Floor", |_lua, this, ()| Ok(Self(this.0.floor())));
        methods.add_method("Sign", |_lua, this, ()| Ok(Self(this.0.signum())));
        methods.add_method("Cross", |_lua, this, (o,): (Vector2,)| {
            Ok(this.0.x * o.0.y - this.0.y * o.0.x)
        });
        methods.add_method(
            "Angle",
            |_lua, this, (o, is_signed): (Vector2, Option<bool>)| {
                let is_signed = is_signed.unwrap_or_default();
                let angle = this.0.angle_to(o.0);
                if is_signed {
                    Ok(angle)
                } else {
                    Ok(angle.abs())
                }
            },
        );
        methods.add_method("Dot", |_lua, this, (o,): (Vector2,)| Ok(this.0.dot(o.0)));
        methods.add_method("FuzzyEq", |_lua, this, (o, e): (Vector2, f64)| {
            Ok(this.0.abs_diff_eq(o.0, e as f32))
        });
        methods.add_method("Lerp", |_lua, this, (o, e): (Vector2, f64)| {
            Ok(Self(this.0.lerp(o.0, e as f32)))
        });
        methods.add_method("Max", |_lua, this, (o,): (Vector2,)| {
            Ok(Self(this.0.max(o.0)))
        });
        methods.add_method("Min", |_lua, this, (o,): (Vector2,)| {
            Ok(Self(this.0.min(o.0)))
        });
        methods.add_meta_method("__add", |_lua, this, (o,): (Vector2,)| {
            Ok(Self(this.0 + o.0))
        });
        methods.add_meta_method("__sub", |_lua, this, (o,): (Vector2,)| {
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
        methods.add_meta_method("__tostring", |_, this, ()| {
            Ok(format!("({}, {})", this.0.x, this.0.y))
        });
    }
}

impl Vector2 {
    pub fn new(_lua: &Lua, (x, y): (f64, f64)) -> LuaResult<Vector2> {
        Ok(Vector2(Vec2 {
            x: x as f32,
            y: y as f32,
        }))
    }
}

#[register]
impl LuaSingleton for Vector2 {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        let table = lua.create_table()?;
        table.raw_set("new", lua.create_function(Vector2::new)?)?;
        table.set_readonly(true);
        lua.globals().raw_set("Vector2", table)?;
        Ok(())
    }
}
