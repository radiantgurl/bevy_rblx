use bevy_rblx_derive::{fast_flag, register_class};
use mlua::prelude::*;
use crate::enums::RunState;
use crate::internal_prelude::*;

use crate::core::{FAST_FLAGS, ServiceMembers, WorldAccess};
use crate::userdata::ObjectRef;

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

        pub is_client: bool,
        pub is_server: bool,

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
    }
}

fast_flag!(FFIsStudio: bool = false);
fast_flag!(FFIsEdit: bool = false);
fast_flag!(FFIsRunMode: bool = false);