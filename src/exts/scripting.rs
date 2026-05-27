use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::{Allow, With};
use bevy::ecs::resource::Resource;
use bevy::platform::collections::HashMap;
use bevy_rblx_derive::{fast_flag, register, register_class};
use mlua::prelude::*;

use crate::core::extension::{EngineExtensionDistribution, EngineExtensionInitLevel};
use crate::core::lua::{FFLuauDefaultJit, LuaSingleton};
use crate::core::object::{DisabledObject, Instance, ObjectHeader};
use crate::core::{FAST_FLAGS, object::InstanceMembers};
use crate::enums::RunContext;
use crate::internal::EngineExtension;
use crate::internal_prelude::*;

use crate::core::{ContainerProvenance, WorldAccess};
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
    } else if source.starts_with("--!no-native") {
        lua.enable_jit(false);
    }
    let env = lua.create_table()?;
    lua.globals()
        .for_each(|k: LuaValue, v: LuaValue| env.raw_set(k, v))
        .unwrap();
    env.raw_set("script", ObjectRef::new(lua, script))?;
    env.set_safeenv(true);
    path.insert(0, '@');
    let res = lua
        .load(source)
        .set_name(path)
        .set_environment(env)
        .into_function();
    lua.enable_jit(FAST_FLAGS.fetch::<FFLuauDefaultJit>());
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
    let mut wa = WorldAccess::fetch(lua);
    let world = wa.access_synchronized()?;
    let mut members = BaseScriptMembers::fetch_members_mut(world, this);
    if members.enabled == new_value {
        return Ok(false);
    }
    members.enabled = new_value;
    let started = members.started;
    drop(members);
    if started == new_value {
        return Ok(true);
    }
    if new_value {

    } else {

    }
    return Ok(true);
}

register_class! {
    #[post_init=fn(lua:&Lua, _this: Entity) -> LuaResult<()> {
        if !WorldAccess::fetch_readonly(lua).access_read_only().contains_resource::<ScriptingLoaded>() {
            Err(LuaError::runtime("Scripting module is not loaded."))
        } else {
            Ok(())
        }
    }]
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
        pub run_context: RunContext,
        priv started: bool
    }
    methods {}
}

#[derive(Default, Resource)]
struct ScriptingLoaded;

#[derive(Default, Clone)]
struct ScriptingExt;
#[register]
impl EngineExtension for ScriptingExt {
    fn id(&self) -> &'static str {
        "scripting"
    }

    fn init_level(&self) -> EngineExtensionInitLevel {
        EngineExtensionInitLevel::Runtime
    }

    fn distribution(&self) -> EngineExtensionDistribution {
        EngineExtensionDistribution::Both
    }

    fn dyn_clone(&mut self, _app: &mut bevy::app::App) -> Box<dyn EngineExtension> {
        Box::new(Self)
    }
    
    fn name(&self) -> &'static str {
        "Scripting"
    }
    
    fn description(&self) -> Option<&'static str> {
        Some("Adds LuaSourceContainer derived classes")
    }

    fn runtime_init(&self, world: &mut bevy::ecs::world::World) {
        world.insert_resource(ScriptingLoaded);
    }
    
    fn post_shutdown_hook(&self, world: &mut bevy::ecs::world::World) {
        let entities = world.query_filtered::<Entity, With<BaseScriptMembers>>().iter(world).collect::<Vec<_>>();
        for e in entities {
            world.entity_mut(e).remove::<BaseScriptMembers>();
        }
    }
}