use bevy::app::Update;
use bevy::ecs::entity::{ContainsEntity, Entity};
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::lifecycle::RemovedComponents;
use bevy::ecs::query::{Added, Allow, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query};
use bevy::ecs::world::World;
use bevy::platform::collections::HashMap;
use bevy_rblx_derive::{cached_lua_function, fast_flag, register, register_class};
use mlua::prelude::*;

use crate::core::extension::{EngineExtensionDistribution, EngineExtensionInitLevel};
use crate::core::lua::{FFLuauDefaultJit, LuaSingleton};
use crate::core::object::{DisabledObject, FFIsEdit, Instance, ObjectHeader, RootInstance};
use crate::core::{FAST_FLAGS, object::InstanceMembers};
use crate::enums::RunContext;
use crate::internal::EngineExtension;
use crate::internal_prelude::*;

use crate::core::{
    ContainerProvenance, Headless, LuauContainer, SchedulerPhase, TaskScheduler, ThreadIdentity,
    WorldAccess,
};
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

#[cached_lua_function]
fn disable_basescript(lua: &Lua, this: ObjectRef) -> LuaResult<()> {
    let mut wa = WorldAccess::fetch(lua);
    let world = wa.access_synchronized()?;
    let threads = ThreadIdentity::get_threads(lua, this.entity());
    let task = TaskScheduler::fetch(lua);
    for i in threads {
        task.cancel(lua, i)?;
    }
    world
        .get_mut::<BaseScriptMembers>(this.entity())
        .unwrap()
        .started = false;
    if let Some(mut p) = world.get_mut::<ContainerProvenance>(this.entity()) {
        p.internally_managed = false;
        drop(p);
        let _ = world.modify_component::<ChildOf, ()>(this.entity(), |_| ()); // Update ChildOf to ensure container provenance is rechecked
    }
    Ok(())
}
#[cached_lua_function]
fn enable_basescript(lua: &Lua, this: ObjectRef) -> LuaResult<()> {
    let mut wa = WorldAccess::fetch(lua);
    let world = wa.access_synchronized()?;
    let is_server = world.contains_resource::<Headless>();
    let is_plugin = FAST_FLAGS.fetch::<FFIsEdit>();
    let script_is_client = world.get::<LocalScriptMembers>(this.entity()).is_some();
    {
        let c = world.get::<ContainerProvenance>(this.entity()).unwrap();
        let c = world.get::<LuauContainer>(c.entity).unwrap();
        assert!(
            c.lua.weak() == lua.weak(),
            "mismatch detected while trying to enable a script"
        );
    }
    let mut members = BaseScriptMembers::fetch_members_mut(world, this.entity());
    match members.run_context {
        RunContext::Legacy => {
            if !(script_is_client ^ is_server) {
                return Ok(());
            }
        }
        RunContext::Server => {
            if !is_server {
                return Ok(());
            }
        }
        RunContext::Client => {
            if is_server || is_plugin {
                return Ok(());
            }
        }
        RunContext::Plugin => {
            if !is_plugin {
                return Ok(());
            }
        }
    }
    if members.started {
        return Ok(());
    }
    members.started = true;
    drop(members);
    let source = LuaSourceContainerMembers::fetch_members(world, this.entity())
        .source
        .clone();
    drop(wa);
    let path = Instance::get_full_name(lua, (this.clone_lua(lua),))?;
    let f = create_lua_function(lua, source, path, this.entity())?;
    TaskScheduler::fetch(lua).defer_next_frame(lua, f, ())?;
    Ok(())
}

fn get_provenance_for_enabling_script(this: Entity, world: &mut World) -> Lua {
    if let Some(mut x) = world.get_mut::<ContainerProvenance>(this.entity()) {
        x.internally_managed = true;
        let e = x.entity;
        drop(x);
        world.get::<LuauContainer>(e).unwrap().lua.clone()
    } else {
        // find first container
        let mut ancestors_qs = world.query_filtered::<&ChildOf, Allow<DisabledObject>>();
        let mut new_lua = None;
        let mut container_entity = None;
        for parent in ancestors_qs.query(world).iter_ancestors(this.entity()) {
            if let Some(container) = world.get::<LuauContainer>(parent) {
                new_lua = Some(container.lua.clone());
                container_entity = Some(parent);
                break;
            }
        }
        if new_lua.is_none() {
            // use the game at this point istg
            let mut root = world.query_filtered::<(Entity, &LuauContainer), With<RootInstance>>();
            let root = root.single(world).unwrap();
            new_lua = Some(root.1.lua.clone());
            container_entity = Some(root.0);
        }
        world.entity_mut(this.entity()).insert(ContainerProvenance {
            entity: container_entity.unwrap(),
            internally_managed: true,
        });
        new_lua.unwrap()
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
    if started == new_value || world.get::<DisabledObject>(this.entity()).is_some() {
        return Ok(true);
    }
    let new_lua = get_provenance_for_enabling_script(this, world);
    drop(wa);
    if new_value {
        TaskScheduler::fetch(&new_lua).defer(
            &new_lua,
            ENABLE_BASESCRIPT.fetch(&new_lua),
            ObjectRef::new(lua, this),
        )?;
    } else {
        TaskScheduler::fetch(&new_lua).defer(
            &new_lua,
            DISABLE_BASESCRIPT.fetch(&new_lua),
            ObjectRef::new(lua, this),
        )?;
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

register_class! {
    Script(BaseScript)
    members {}
    methods {}
}

register_class! {
    LocalScript(Script)
    members {}
    methods {}
}

#[derive(Default, Resource)]
struct ScriptingLoaded;

fn script_enable_disable(
    mut commands: Commands,

    just_enabled: Query<Entity, (Without<DisabledObject>, With<BaseScriptMembers>)>,
    mut removed_disabled_object: RemovedComponents<DisabledObject>,
    just_disabled: Query<Entity, (Added<DisabledObject>, With<BaseScriptMembers>)>,

    members: Query<&BaseScriptMembers, Allow<DisabledObject>>,
) {
    for e in removed_disabled_object.read() {
        if just_enabled.contains(e) && members.get(e).unwrap().enabled {
            // schedule for enabling
            commands.queue(move |w: &mut World| {
                let lua = get_provenance_for_enabling_script(e, w);
                unsafe {
                    w.entity_mut(e).inc_ref();
                }
                TaskScheduler::fetch(&lua)
                    .defer_next_frame_custom_pd(
                        &lua,
                        ENABLE_BASESCRIPT.fetch(&lua),
                        unsafe { ObjectRef::new_no_inc_ref(&lua, e) },
                        false,
                    )
                    .unwrap();
            });
        }
    }
    for e in just_disabled.iter() {
        let m = members.get(e).unwrap();
        if m.started {
            // schedule for disabling
            commands.queue(move |w: &mut World| {
                let lua = get_provenance_for_enabling_script(e, w);
                unsafe {
                    w.entity_mut(e).inc_ref();
                }
                TaskScheduler::fetch(&lua)
                    .defer_next_frame_custom_pd(
                        &lua,
                        DISABLE_BASESCRIPT.fetch(&lua),
                        unsafe { ObjectRef::new_no_inc_ref(&lua, e) },
                        false,
                    )
                    .unwrap();
            });
        }
    }
}

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
        world.schedule_scope(Update, |_, s| {
            s.add_systems(script_enable_disable.before(SchedulerPhase::PreHeartbeat));
        });
    }

    fn post_shutdown_hook(&self, world: &mut bevy::ecs::world::World) {
        let entities = world
            .query_filtered::<Entity, With<BaseScriptMembers>>()
            .iter(world)
            .collect::<Vec<_>>();
        for e in entities {
            world.entity_mut(e).remove::<BaseScriptMembers>();
        }
    }
}
