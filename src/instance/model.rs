use bevy_rblx_derive::register_class;

use mlua::prelude::*;
use bevy::prelude::*;
use crate::enums::{ModelLevelOfDetail, ModelStreamingMode};
use crate::internal_prelude::*;

use crate::core::{InstanceMembers, WorldAccess};
use crate::userdata::{CFrame, ObjectRef};

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