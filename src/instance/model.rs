use bevy::platform::collections::HashMap;
use bevy_rblx_derive::register_class;
use parking_lot::Mutex;

use crate::core::LuauContainer;
use crate::core::templates::EraseOnClone;
use crate::enums::{ModelLevelOfDetail, ModelStreamingMode};
use crate::internal_prelude::*;
use bevy::prelude::*;
use mlua::prelude::*;

use crate::core::{WorldAccess, object::InstanceMembers};
use crate::userdata::{CFrame, LuaFreeValue, ObjectRef, RBXScriptConnection, RBXScriptSignal};

register_class! {
    abstract PVInstance(Instance)
    members {
        pub priv origin: CFrame,
        #[rename="Pivot Offset"]
        pub priv pivot_offset: CFrame
    }
    methods {
        fn get_pivot(lua: &Lua, this: ObjectRef) -> LuaResult<CFrame> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            let members = PVInstanceMembers::fetch_members(&*world, this.entity());
            let piv_offset: Transform = members.pivot_offset.into();
            let origin: Transform = members.origin.into();
            let cf: CFrame = piv_offset.mul_transform(origin).into();
            Ok(cf)
        }
        fn pivot_to(lua: &Lua, this: ObjectRef, cf: CFrame) -> LuaResult<()> {
            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;
            let mut members = PVInstanceMembers::fetch_members_mut(world, this.entity());
            let piv_offset: Transform = members.pivot_offset.into();
            let piv_offset_inverse = Transform::from_matrix(piv_offset.to_matrix().inverse());
            members.origin = piv_offset_inverse.mul_transform(cf.into()).into();
            Ok(())
        }
    }
}
register_class! {
    Model(PVInstance)
    members {
        #[security=PLUGIN]
        pub level_of_detail: ModelLevelOfDetail,
        pub model_streaming_mode: ModelStreamingMode,
        // priv world_pivot_set: bool,
        // virtual world_pivot: CFrame,
        primary_part: Option<ObjectRef>
    }
    methods {}
}

register_class! {
    #[require_components(LuauContainer)]
    Actor(Model)
    members {
        #[reflect_opaque]
        priv messages: EraseOnClone<Mutex<HashMap<String, RBXScriptSignal>>>
    }
    methods {
        fn bind_to_message(lua: &Lua, this: ObjectRef, topic: String, function: LuaFunction) -> LuaResult<RBXScriptConnection> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();

            let mut guard = ActorMembers::fetch_members(&*world, this.entity()).messages.lock();
            guard.entry(topic).or_default().connect(lua, function)
        }
        fn bind_to_message_parallel(lua: &Lua, this: ObjectRef, topic: String, function: LuaFunction) -> LuaResult<RBXScriptConnection> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();

            let mut guard = ActorMembers::fetch_members(&*world, this.entity()).messages.lock();
            guard.entry(topic).or_default().connect_parallel(lua, function)
        }
        fn send_message(lua: &Lua, this: ObjectRef, topic: String, values: LuaMultiValue) -> LuaResult<()> {
            let mut free_values = Vec::new();
            for (idx, i) in values.into_iter().enumerate() {
                free_values.push(LuaFreeValue::from_lua(i, lua).map_err(|e| e.context(format!("while parsing arg {}", idx+1)))?);
            }
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();

            let guard = ActorMembers::fetch_members(&*world, this.entity()).messages.lock();
            if let Some(ev) = guard.get(&topic) {
                ev.fire_in_lua(lua, false, free_values)?;
            }
            Ok(())
        }
    }
}
