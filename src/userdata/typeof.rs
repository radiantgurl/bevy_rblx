use crate::userdata::ObjectRef;
use crate::{
    core::{
        FAST_FLAGS, WorldAccess,
        lua::{FFLuauDefaultJit, LuaSingleton},
        object::ObjectHeader,
    },
    internal_prelude::*,
};
use bevy_rblx_derive::register;
use mlua::prelude::*;

pub struct TypeOfFn;

#[register]
impl LuaSingleton for TypeOfFn {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.enable_jit(true);

        let old_typeof = lua.globals().raw_get::<LuaFunction>("typeof")?;

        let new_typeof =
            lua.create_function(move |lua: &Lua, v: LuaValue| -> LuaResult<LuaValue> {
                let res = old_typeof.call::<LuaString>(v.clone())?;

                if res.to_string_lossy() == "Object" {
                    let obj = ObjectRef::from_lua(v, lua)?;
                    let world_access = WorldAccess::fetch_readonly(lua);
                    let world = world_access.access_read_only();
                    if world
                        .get::<ObjectHeader>(obj.entity())
                        .expect("is object")
                        .vtable
                        .is_a("Instance")
                    {
                        "Instance".into_lua(lua)
                    } else {
                        "Object".into_lua(lua)
                    }
                } else {
                    res.into_lua(lua)
                }
            })?;

        lua.globals().raw_set("typeof", new_typeof)?;

        lua.enable_jit(FAST_FLAGS.fetch::<FFLuauDefaultJit>());

        Ok(())
    }
}
