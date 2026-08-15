use super::PVInstanceMembers;
use crate::{
    core::{Headless, WorldAccess, push_log},
    enums::{CameraType, FieldOfViewType},
    internal_prelude::*,
    userdata::{CFrame, ObjectRef},
};
use bevy::{
    camera::{Camera3d, PerspectiveProjection, Projection},
    ecs::entity::Entity,
    utils::default,
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
            push_log(lua, crate::enums::MessageType::MessageWarning, lua.traceback(Some("only vertical FOV is currently implemented"), 2)?.to_string_lossy());
            Ok(())
        }]
        #[default=FieldOfViewType::Vertical]
        virtual field_of_view_type: FieldOfViewType,
        #[getter=fn(lua: &Lua, this: Entity, vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            LuaFunction::from_lua(vtable.get(lua, this, "GetPivot".into())?, lua)?.call(ObjectRef::new(lua, this))
        }]
        virtual focus: CFrame,
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
        #[setter=fn(lua: &Lua, this: Entity, ctx: &mut ObjectContext, value: LuaValue) -> LuaResult<()> {
            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;
            match world.get_mut::<Projection>(this).unwrap().as_mut() {
                Projection::Perspective(v) =>
                    v.fov = f32::from_lua(value, lua)?
                ,
                _ => unimplemented!(),
            }
            Ok(())
        }]
        #[default=70.0]
        virtual field_of_view: f64,
        #[default=(-0.1)]
        #[read_only]
        pub near_plane_z: f64,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            lua_todo!()
        }]
        virtual viewport_size: Vector2
    }
    methods {

    }
}
