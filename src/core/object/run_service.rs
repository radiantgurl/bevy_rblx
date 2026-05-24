use crate::core::{Headless, TaskScheduler};
use crate::core::lua::{callback::LuaPrioCallbackTableCached, CachedLuaFunction};
use crate::enums::{PredictionMode, PredictionStatus, RunState, StepFrequency};
use crate::internal_prelude::*;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::system::Local;
use bevy::ecs::world::World;
use bevy::platform::collections::HashMap;
use bevy::time::{Fixed, Time};
use bevy_rblx_derive::{fast_flag, register_class};
use mlua::prelude::*;

use crate::core::{FAST_FLAGS, WorldAccess, object::ServiceMembers};
use crate::userdata::{ObjectRef, RBXScriptConnection, RBXScriptSignal};

const SIMULATION_IS_CONNECTED: CachedLuaFunction = {
    fn simulation_is_connected(lua: &Lua, (this, id): (ObjectRef, usize)) -> LuaResult<bool> {
        let wa = WorldAccess::fetch_readonly(lua);
        let world = wa.access_read_only();

        Ok(RunServiceMembers::fetch_members(&*world, this.entity()).simulation_callbacks.contains(id))
    }
    fn create_fn(lua: &Lua) -> LuaFunction {
        lua.create_function(simulation_is_connected).unwrap()
    }
    CachedLuaFunction::new(create_fn)
};
const SIMULATION_DISCONNECT: CachedLuaFunction = {
    fn simulation_disconnect(lua: &Lua, (this, id): (ObjectRef, usize)) -> LuaResult<()> {
        let mut wa = WorldAccess::fetch(lua);
        let world = wa.access_synchronized()?;

        let mut m = RunServiceMembers::fetch_members_mut(world, this.entity());
        m.simulation_callbacks.remove(id);
        Ok(())
    }
    fn create_fn(lua: &Lua) -> LuaFunction {
        lua.create_function(simulation_disconnect).unwrap()
    }
    CachedLuaFunction::new(create_fn)
};

register_class! {
    priv RunService (Service)
    members {
        #[getter=fn(lua: &Lua, _this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            env!("GIT_HASH").into_lua(lua)
        }]
        virtual client_git_hash: String,

        #[security=PLUGIN]
        #[default=RunState::Running]
        run_state: RunState,

        pub priv is_client: bool,
        pub priv is_server: bool,

        #[read_only]
        pub heartbeat: RBXScriptSignal,
        #[read_only]
        pub misprediction: RBXScriptSignal,
        #[read_only]
        pub post_simulation: RBXScriptSignal,
        #[read_only]
        pub pre_animation: RBXScriptSignal,
        #[read_only]
        #[deprecated_alias="RenderStepped"]
        pub pre_render: RBXScriptSignal,
        #[read_only]
        pub pre_simulation: RBXScriptSignal,
        #[read_only]
        pub rollback: RBXScriptSignal,
        #[read_only]
        pub stepped: RBXScriptSignal,

        pub(super) priv render_callbacks: LuaPrioCallbackTableCached<true, ()>,
        pub(super) priv simulation_callbacks: LuaPrioCallbackTableCached<true, StepFrequency>,

        pub(super) priv render_ids: HashMap<String, usize>,
    }
    methods {
        fn is_running(lua: &Lua, this: ObjectRef) -> LuaResult<bool> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            Ok(world.get::<RunServiceMembers>(this.entity()).expect("is run service").run_state != RunState::Stopped)
        }
        fn is_client(lua: &Lua, this: ObjectRef) -> LuaResult<bool> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            Ok(world.get::<RunServiceMembers>(this.entity()).expect("is run service").is_client)
        }
        fn is_server(lua: &Lua, this: ObjectRef) -> LuaResult<bool> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            Ok(world.get::<RunServiceMembers>(this.entity()).expect("is run service").is_server)
        }
        fn is_studio(lua: &Lua, _this: ObjectRef) -> LuaResult<bool> {
            Ok(FAST_FLAGS.fetch::<FFIsStudio>())
        }
        fn is_edit(lua: &Lua, _this: ObjectRef) -> LuaResult<bool> {
            Ok(FAST_FLAGS.fetch::<FFIsEdit>())
        }
        fn is_run_mode(lua: &Lua, _this: ObjectRef) -> LuaResult<bool> {
            Ok(FAST_FLAGS.fetch::<FFIsRunMode>())
        }
        #[security=PLUGIN]
        fn pause(lua: &Lua, _this: ObjectRef) -> LuaResult<()> {
            lua_todo!()
        }
        #[security=PLUGIN]
        fn run(lua: &Lua, _this: ObjectRef) -> LuaResult<()> {
            lua_todo!()
        }
        #[security=PLUGIN]
        fn stop(lua: &Lua, _this: ObjectRef) -> LuaResult<()> {
            lua_todo!()
        }
        fn get_prediction_status(lua: &Lua, _this: ObjectRef, _context: ObjectRef) -> LuaResult<PredictionStatus> {
            Ok(PredictionStatus::None)
        }
        fn set_prediction_mode(lua: &Lua, _this: ObjectRef, _context: ObjectRef, _mode: PredictionMode) -> LuaResult<()> {
            lua_todo!()
        }
        fn bind_to_render_step(lua: &Lua, this: ObjectRef, name: String, priority: i64, func: LuaFunction) -> LuaResult<()> {
            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;
            let mut members = RunServiceMembers::fetch_members_mut(world, this.entity());

            let new_id = members.render_callbacks.insert(lua, priority, func, ())?;
            if let Some(old_id) = members.render_ids.insert(name, new_id) {
                members.render_callbacks.remove(old_id);
            }
            Ok(())
        }
        fn bind_to_simulation(lua: &Lua, this: ObjectRef, func: LuaFunction, freq: StepFrequency, priority: i64) -> LuaResult<RBXScriptConnection> {
            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;
            let mut members = RunServiceMembers::fetch_members_mut(world, this.entity());
            
            let id = members.simulation_callbacks.insert(lua, priority, func, freq)?;

            drop(members);
            drop(wa);
            let is_connected_fn = SIMULATION_IS_CONNECTED.fetch(lua).bind((this.clone(), id))?;
            let disconnect_fn = SIMULATION_DISCONNECT.fetch(lua).bind((this.clone(), id))?;
            let signal = RBXScriptConnection::new_custom(is_connected_fn, disconnect_fn);
            Ok(signal)
        }
        fn unbind_from_render_step(lua: &Lua, this: ObjectRef, name: String) -> LuaResult<()> {
            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;
            let mut members = RunServiceMembers::fetch_members_mut(world, this.entity());

            if let Some(old_id) = members.render_ids.remove(&name) {
                members.render_callbacks.remove(old_id);
            }
            Ok(())
        }
    }
}

impl RunService {
    pub(in crate::core) fn simulation_hook(w: &mut World, mut frame_count: Local<u8>) {
        let dt = w.resource::<Time<Fixed>>().delta_secs_f64();
        let run_service = w.query_filtered::<Entity, With<RunServiceMembers>>().single(w).unwrap();
        for (lua, func, freq) in RunServiceMembers::fetch_members_mut(w, run_service).simulation_callbacks.get_callbacks_cached() {
            let should_run = match freq {
                StepFrequency::Hz60 => true,
                StepFrequency::Hz30 => (*frame_count)%2==0,
                StepFrequency::Hz15 => (*frame_count)%4==0,
                StepFrequency::Hz10 => (*frame_count)%6==0,
                StepFrequency::Hz5 => (*frame_count)%12==0,
                StepFrequency::Hz1 => (*frame_count)%60==0,
            };
            if should_run {
                unsafe {
                    WorldAccess::fetch(&lua).insert_sync_access(w);
                }
                TaskScheduler::fetch(&lua).spawn(&lua, func, dt).unwrap();
                WorldAccess::fetch(&lua).clear_sync_access(w);
            }
        }
        *frame_count = (*frame_count + 1)%60;
    }
    pub(in crate::core) fn render_hook(w: &mut World, mut frame_count: Local<u8>) {
        if w.get_resource::<Headless>().is_some() {return;}
        let dt = w.resource::<Time<Fixed>>().delta_secs_f64();
        let run_service = w.query_filtered::<Entity, With<RunServiceMembers>>().single(w).unwrap();
        for (lua, func, _) in RunServiceMembers::fetch_members_mut(w, run_service).render_callbacks.get_callbacks_cached() {
            unsafe {
                WorldAccess::fetch(&lua).insert_sync_access(w);
            }
            TaskScheduler::fetch(&lua).spawn(&lua, func, dt).unwrap();
            WorldAccess::fetch(&lua).clear_sync_access(w);
        }
        *frame_count = (*frame_count + 1)%60;
    }
}

fast_flag!(FFIsStudio: bool = false);
fast_flag!(FFIsEdit: bool = false);
fast_flag!(FFIsRunMode: bool = false);
