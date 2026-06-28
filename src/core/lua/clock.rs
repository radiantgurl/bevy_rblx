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
pub fn system_time() -> UnixTimestamp {
    Utc::now().timestamp()
}
pub type UnixTimestamp = i64;
pub type SinceStatic = f64;

struct ClockSingleton;

#[register]
impl LuaSingleton for ClockSingleton {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        let clock_f = lua.create_function(|_, ()| Ok(clock().as_secs_f64()))?;
        lua.globals()
            .raw_get::<LuaTable>("os")?
            .raw_set("clock", clock_f.clone())?;
        #[cfg(feature = "deprecated")]
        lua.globals().raw_set("elapsedTime", clock_f.clone())?;
        Ok(())
    }
}
