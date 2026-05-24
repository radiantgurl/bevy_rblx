use bevy_rblx_derive::register_class;
use mlua::prelude::*;

use super::InstanceMembers;
use crate::{
    core::{
        WorldAccess,
        object::{Instance, OBJECT_VTABLES},
    },
    internal_prelude::*,
    userdata::{ObjectRef, RBXScriptSignal, instance_new},
};

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
            if let Some(service) = Instance::find_first_child_of_class(lua, (this.clone_lua(lua), class_name.clone()))? {
                return Ok(Some(service));
            }
            let vtable = OBJECT_VTABLES.get(class_name.as_str()).ok_or_else(|| LuaError::runtime("invalid class"))?;

            if vtable.is_a("Service") {
                let entity = instance_new(lua, class_name)?;
                let service_added = {
                    let wa = WorldAccess::fetch_readonly(lua);
                    let world = wa.access_read_only();
                    ServiceProviderMembers::fetch_members(&*world, this.entity()).service_added.reference()
                };
                service_added.fire_in_lua(lua, true, entity.clone_lua(lua))?;
                Ok(Some(entity))
            } else {
                Ok(None) // not a service
            }
        }
    }
}
