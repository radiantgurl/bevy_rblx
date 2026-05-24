use crate::core::lua::callback::LuaPrioCallbackTableCached;
use crate::enums::{PredictionMode, PredictionStatus, RunState, StepFrequency};
use crate::internal_prelude::*;
use bevy::platform::collections::HashMap;
use bevy_rblx_derive::{fast_flag, register_class};
use mlua::prelude::*;

use crate::core::{FAST_FLAGS, WorldAccess, object::ServiceMembers};
use crate::userdata::{ObjectRef, RBXScriptConnection, RBXScriptSignal};

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

        pub(super) priv render_callbacks: LuaPrioCallbackTableCached<true>,
        pub(super) priv simulation_callbacks: LuaPrioCallbackTableCached<true>,

        pub(super) priv render_ids: HashMap<String, usize>,
        pub(super) priv simulation_callback_ids: HashMap<String, (usize, StepFrequency)>
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

            let new_id = members.render_callbacks.insert(lua, priority, func)?;
            if let Some(old_id) = members.render_ids.insert(name, new_id) {
                members.render_callbacks.remove(old_id);
            }
            Ok(())
        }
        fn bind_to_simulation(lua: &Lua, this: ObjectRef, func: LuaFunction, freq: StepFrequency, priority: i64) -> LuaResult<RBXScriptConnection> {
            lua_todo!()
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

impl RunService {}

fast_flag!(FFIsStudio: bool = false);
fast_flag!(FFIsEdit: bool = false);
fast_flag!(FFIsRunMode: bool = false);
