use std::time::{Duration, Instant};

use bevy_rblx_derive::register;
use chrono::Utc;
use lazy_static::lazy_static;
use mlua::prelude::*;

use crate::{core::lua::singleton::LuaSingleton, internal_prelude::*};

lazy_static! {
    static ref CLOCK: Instant = Instant::now();
}

pub fn clock() -> Duration {
    CLOCK.elapsed()
}
pub fn system_time() -> i64 {
    Utc::now().timestamp()
}

struct ClockSingleton;

#[register]
impl LuaSingleton for ClockSingleton {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.globals().raw_get::<LuaTable>("os")?.raw_set(
            "clock",
            lua.create_function(|_, ()| Ok(clock().as_secs_f64()))?,
        )
    }
}
