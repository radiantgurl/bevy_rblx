use bevy_rblx_derive::register_class;
use mlua::prelude::*;

use super::InstanceMembers;
use crate::{core::Instance, internal_prelude::*, userdata::{ObjectRef, RBXScriptSignal}};

register_class! {
    abstract ServiceProvider(Instance)
    members {
        #[read_only]
        pub close: RBXScriptSignal,
        #[read_only]
        pub service_added: RBXScriptSignal,
        #[read_only]
        pub service_removing: RBXScriptSignal
    }
    methods {
        fn find_service(lua: &Lua, this: ObjectRef, class_name: String) -> LuaResult<Option<ObjectRef>> {
            // let wa = WorldAccess::fetch_readonly(lua);
            // let world = wa.access_read_only();

            Instance::find_first_child_of_class(lua, (this, class_name))
        }
        fn get_service(lua: &Lua, this: ObjectRef, class_name: String) -> LuaResult<Option<ObjectRef>> {
            lua_todo!()
        }
    }
}
