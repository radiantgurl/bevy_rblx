use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::platform::collections::HashMap;
use bevy_rblx_derive::{fast_flag, register, register_class};
use mlua::prelude::*;

use crate::core::instance::RootInstance;
use crate::core::luau::FFLuauForceJit;
use crate::core::{FAST_FLAGS, Instance, InstanceMembers, LuaSingleton, ObjectHeader};
use crate::enums::RunContext;
use crate::internal_prelude::*;

use crate::core::WorldAccess;
use crate::userdata::ObjectRef;

register_class! {
    abstract LuaSourceContainer (Instance)
    members {
        #[security=PLUGIN]
        pub source: String
    }
    methods {}
}

#[derive(Default)]
pub struct ModuleScriptTable {
    module_scripts: HashMap<Entity, LuaValue>,
    require_stack: Vec<Entity>,
}

#[register]
impl LuaSingleton for ModuleScriptTable {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.set_app_data(ModuleScriptTable::default());
        Ok(())
    }
}

register_class! {
    ModuleScript (LuaSourceContainer)
    members {}
    methods {}
}

fast_flag!(FFLuauDisableNativeFlag: bool = false);

fn create_lua_function(
    lua: &Lua,
    source: String,
    mut path: String,
    script: Entity,
) -> LuaResult<LuaFunction> {
    if source.starts_with("--!native") && !FAST_FLAGS.fetch::<FFLuauDisableNativeFlag>() {
        lua.enable_jit(true);
    }
    let game_root = {
        let wa = WorldAccess::fetch_readonly(lua);
        let world = wa.access_read_only();
        world
            .try_query_filtered::<Entity, With<RootInstance>>()
            .expect("failed to create game root query :(")
            .single(&*world)
            .unwrap()
    };
    let env = lua.create_table()?;
    lua.globals().for_each(|k: LuaValue,v: LuaValue| env.raw_set(k,v)).unwrap();
    env.raw_set("game", ObjectRef::new(lua, game_root))?;
    env.raw_set("script", ObjectRef::new(lua, script))?;
    path.insert(0, '@');
    let res = lua
        .load(source)
        .set_name(path)
        .set_environment(env)
        .into_function();
    lua.enable_jit(FAST_FLAGS.fetch::<FFLuauForceJit>());
    res
}

fn require(lua: &Lua, o: LuaValue) -> LuaResult<LuaValue> {
    if let Some(s) = o.as_string() {
        let _s = s.to_string_lossy();
        lua_todo!("Cannot import using rbx asset id yet!")
    } else {
        let e = {
            let oref: LuaUserDataRef<ObjectRef> = o.borrow_typed()?;
            oref.entity()
        };
        if let Some(v) = lua
            .app_data_ref::<ModuleScriptTable>()
            .unwrap()
            .module_scripts
            .get(&e)
        {
            Ok(v.clone())
        } else {
            let path = Instance::get_full_name(lua, (ObjectRef::new(lua, e),))?;
            lua.app_data_mut::<ModuleScriptTable>()
                .unwrap()
                .require_stack
                .push(e);
            let source = {
                let wa = WorldAccess::fetch_readonly(lua);
                let world = wa.access_read_only();
                let vtable = world.get::<ObjectHeader>(e).unwrap().vtable;
                if !vtable.is_a("ModuleScript") {
                    return Err(LuaError::runtime(format!(
                        "expected ModuleScript, got {}",
                        vtable.class_name
                    )));
                }
                let m = LuaSourceContainerMembers::fetch_members(&*&world, e);
                m.source.clone()
            };
            let f = create_lua_function(lua, source, path.clone(), e)
                .and_then(|f| {
                    let thr = lua.create_thread(f)?;
                    thr.resume::<LuaValue>(())
                })
                .map_err(|x| {
                    let traceback = lua.traceback(None, 2).unwrap().to_string_lossy();
                    lua.app_data_mut::<ModuleScriptTable>()
                        .unwrap()
                        .require_stack
                        .pop();
                    x.context(format!(
                        "error occured while requiring script {path}\n{traceback}"
                    ))
                })?;
            lua.app_data_mut::<ModuleScriptTable>()
                .unwrap()
                .require_stack
                .pop();
            let mut table = lua.app_data_mut::<ModuleScriptTable>().unwrap();
            table.module_scripts.insert(e, f.clone());
            Ok(f)
        }
    }
}

#[register]
impl LuaSingleton for ModuleScript {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.globals()
            .raw_set("require", lua.create_function(require)?)
    }
}

fn set_enabled(lua: &Lua, this: Entity, new_value: bool) -> LuaResult<bool> {
    {
        let mut wa = WorldAccess::fetch(lua);
        let world = wa.access_synchronized()?;
        let mut members = BaseScriptMembers::fetch_members_mut(world, this);
        if members.enabled == new_value {
            return Ok(false);
        }
        members.enabled = new_value;
    }
    todo!();
    if new_value {
    } else {
    }
    Ok(true)
}

register_class! {
    abstract BaseScript (LuaSourceContainer)
    members {
        #[setter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable, value: LuaValue) -> LuaResult<bool> {
            let new_value = bool::from_lua(value, lua)?;
            set_enabled(lua, this, new_value)
        }]
        enabled: bool,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();

            (!world.get::<BaseScriptMembers>(this).unwrap().enabled).into_lua(lua)
        }]
        #[setter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable, value: LuaValue) -> LuaResult<bool> {
            let new_value = bool::from_lua(value, lua)?;
            set_enabled(lua, this, !new_value)
        }]
        virtual disabled: bool,
        pub run_context: RunContext
    }
    methods {}
}
