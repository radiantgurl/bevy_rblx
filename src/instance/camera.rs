use super::PVInstanceMembers;
use crate::{
    core::{Headless, WorldAccess, push_log}, enums::{CameraType, FieldOfViewType}, internal_prelude::*, userdata::{CFrame, ObjectRef},
};
use bevy::{
    camera::{Camera2d, Camera3d, PerspectiveProjection, Projection}, ecs::entity::Entity, utils::default,
};
use bevy_rblx_derive::register_class;
use mlua::prelude::*;

register_class! {
    #[post_init=fn(lua: &Lua, this: Entity) -> LuaResult<()> {
        let mut wa = WorldAccess::fetch(lua);
        let (w,mut c) = wa.access_world_commands();
        if !w.contains_resource::<Headless>() {
            c.entity(this).insert((Camera3d::default(), Projection::Perspective(PerspectiveProjection {
                fov: 70f32.to_radians(),
                ..default()
            })));
        }
        Ok(())
    }]
    Camera (PVInstance)
    members {
        pub camera_subject: Option<ObjectRef>,
        #[default=CameraType::Fixed]
        pub camera_type: CameraType,
        #[getter=fn(lua: &Lua, this: Entity, vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            LuaFunction::from_lua(vtable.get(lua, this, "GetPivot".into())?, lua)?.call(ObjectRef::new(lua, this))
        }]
        #[setter=fn(lua: &Lua, this: Entity, ctx: &mut ObjectContext, v: LuaValue) -> LuaResult<()> {
            LuaFunction::from_lua(ctx.vtable().get(lua, this, "PivotTo".into())?, lua)?.call((ObjectRef::new(lua, this), v))
        }]
        virtual c_frame: CFrame,
        #[getter=fn(lua: &Lua, _this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            FieldOfViewType::Diagonal.into_lua(lua)
        }]
        #[setter=fn(lua: &Lua, _this: Entity, _ctx: &mut ObjectContext, _v: LuaValue) -> LuaResult<()> {
            push_log(lua, crate::enums::MessageType::MessageWarning, lua.traceback(Some("only vertial FOV is currently implemented"), 2)?.to_string_lossy());
            Ok(())
        }]
        #[default=FieldOfViewType::Vertical]
        virtual field_of_view_type: FieldOfViewType,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            match world.get::<Projection>(this).unwrap() {
                Projection::Perspective(v) => 
                    v.fov.to_degrees().into_lua(lua)
                ,
                _ => unimplemented!(),
            }
        }]
        #[default=70.0]
        virtual field_of_view: f64,
        #[default=(-0.1)]
        pub near_plane_z: f64
    }
    methods {

    }
}
