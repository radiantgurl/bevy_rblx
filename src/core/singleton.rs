use mlua::prelude::*;

pub trait LuaSingleton: Sized {
    fn register_singleton(lua: &Lua) -> LuaResult<()>;
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct SingletonRegisterFn(pub fn(&Lua) -> LuaResult<()>);

inventory::collect!(SingletonRegisterFn);

pub(crate) fn init_singletons(l: &Lua) -> LuaResult<()> {
    for i in inventory::iter::<SingletonRegisterFn> {
        i.0(l)?;
    }
    Ok(())
}
