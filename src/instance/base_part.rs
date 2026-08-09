use bevy_rblx_derive::register_class;

use crate::{
    core::{WorldAccess, object::InstanceMembers},
    enums::{Material, NormalId},
    internal_prelude::*,
    userdata::{CFrame, Color3, ObjectRef, RBXScriptSignal, Vector3},
};
use bevy::prelude::*;
use mlua::prelude::*;

#[derive(Clone, Copy, Default, Debug, Reflect)]
pub enum NetworkOwnership {
    #[default]
    Server,
    Client(Entity),
}
// todo: finalize impl
register_class! {
    #[require_components(Transform)]
    abstract BasePart(Instance)
    members {
        pub anchored: bool,
        pub locked: bool,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            CFrame::from(*world.get::<Transform>(this).unwrap()).into_lua(lua)
        }]
        #[setter=fn(lua: &Lua, this: Entity, ctx: &mut ObjectContext, value: LuaValue) -> LuaResult<()> {
            let cf = Into::<Transform>::into(CFrame::from_lua(value, lua)?);

            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;

            let mut transform = world.get_mut::<Transform>(this).unwrap();

            if cf != *transform {
                *transform = cf;
                ctx.set_changed();
            }
            Ok(())
        }]
        #[changed_aliases=["Position", "Orientation", "Rotation"]]
        #[rename="CFrame"]
        virtual cframe: CFrame,
        #[default=Vector3::from(Vec3::new(2.0,1.0,4.0))]
        pub size: Vector3,
        pub pivot_offset: CFrame,

        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            Vector3::from(world.get::<Transform>(this).unwrap().translation).into_lua(lua)
        }]
        #[setter=fn(lua: &Lua, this: Entity, ctx: &mut ObjectContext, v: LuaValue) -> LuaResult<()> {
            let value = Into::<Vec3>::into(Vector3::from_lua(v, lua)?);

            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;

            let mut transform = world.get_mut::<Transform>(this).unwrap();
            if transform.translation != value {
                ctx.set_changed();
                transform.translation = value;
            }
            Ok(())
        }]
        #[changed_aliases=["CFrame"]]
        virtual position: Vector3,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            Vector3::from(Vec3::from(world.get::<Transform>(this).unwrap().rotation.to_euler(EulerRot::YXZ))).into_lua(lua)
        }]
        #[changed_aliases=["CFrame", "Rotation"]]
        virtual orientation: Vector3,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            Vector3::from(Vec3::from(world.get::<Transform>(this).unwrap().rotation.to_euler(EulerRot::XYZ))).into_lua(lua)
        }]
        #[changed_aliases=["CFrame", "Orientation"]]
        virtual rotation: Vector3,

        #[default=true]
        pub priv auto_network_ownership: bool,
        pub priv network_ownership: NetworkOwnership,

        #[default=true]
        pub can_collide: bool,
        #[default=true]
        pub can_touch: bool,
        #[default=true]
        pub can_query: bool,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            let v: Vec3 = BasePartMembers::fetch_members(&world, this).size.into();
            Vector3::from(v / 2.0).into_lua(lua)
        }]
        virtual center_of_mass: Vector3,
        pub collision_group: String,
        pub massless: bool,

        pub audio_can_collide: bool,

        #[default=true]
        pub cast_shadow: bool,
        #[setter=fn(lua: &Lua, this: Entity, ctx: &mut ObjectContext, value: LuaValue) -> LuaResult<()> {
            let new_value = f64::from_lua(value, lua)?.min(1.0).max(0.0);
            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;

            let mut members = BasePartMembers::fetch_members_mut(world, this);
            if members.reflectance != new_value {
                members.reflectance = new_value;
                ctx.set_changed();
            }
            Ok(())
        }]
        pub reflectance: f64,
        #[setter=fn(lua: &Lua, this: Entity, ctx: &mut ObjectContext, value: LuaValue) -> LuaResult<()> {
            let new_value = f64::from_lua(value, lua)?.min(1.0).max(0.0);
            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;

            let mut members = BasePartMembers::fetch_members_mut(world, this);
            if members.transparency != new_value {
                members.transparency = new_value;
                ctx.set_changed();
            }
            Ok(())
        }]
        pub transparency: f64,
        #[default=Color3 {r: 232.0 / 255.0, g: 232.0 / 255.0, b: 232.0 / 255.0}]
        pub color: Color3,
        #[default=Material::SmoothPlastic]
        pub material: Material,

        #[read_only]
        pub touched: RBXScriptSignal,
        #[read_only]
        pub touch_ended: RBXScriptSignal
    }
    methods {
        fn angular_acceleration_to_torque(lua: &Lua, this: ObjectRef, ang_acceleration: Vector3, ang_velocity: Vector3) -> LuaResult<Vector3> {
            lua_todo!()
        }
        fn apply_angular_impulse(lua: &Lua, this: ObjectRef, impulse: Vector3) -> LuaResult<()> {
            lua_todo!()
        }
        fn apply_impulse(lua: &Lua, this: ObjectRef, impulse: Vector3) -> LuaResult<()> {
            lua_todo!()
        }
        fn apply_impulse_at_position(lua: &Lua, this: ObjectRef, impulse: Vector3, position: Vector3) -> LuaResult<()> {
            lua_todo!()
        }
        // #[deprecated_alias="breakJoints"]
        // fn break_joints
        fn can_collide_with(lua: &Lua, this: ObjectRef, other: ObjectRef) -> LuaResult<bool> {
            lua_todo!()
        }
        fn can_set_network_ownership(lua: &Lua, this: ObjectRef) -> LuaResult<(bool, Option<String>)> {
            return Ok((false, Some("not implemented yet".into())))
        }
        fn get_closest_point_on_surface(lua: &Lua, this: ObjectRef, position: Vector3) -> LuaResult<Vector3> {
            lua_todo!()
        }
        fn get_connected_parts(lua: &Lua, this: ObjectRef, recursive: bool) -> LuaResult<Vec<ObjectRef>> {
            lua_todo!()
        }
        fn get_joints(lua: &Lua, this: ObjectRef) -> LuaResult<Vec<ObjectRef>> {
            lua_todo!()
        }
        #[deprecated_alias="getMass"]
        fn get_mass(lua: &Lua, this: ObjectRef) -> LuaResult<f64> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            // world.
            lua_todo!()
        }
        fn get_network_owner(lua: &Lua, this: ObjectRef) -> LuaResult<Option<ObjectRef>> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            match BasePartMembers::fetch_members(&world, this.entity()).network_ownership.clone() {
                NetworkOwnership::Server => Ok(None),
                NetworkOwnership::Client(entity) => Ok(Some(ObjectRef::new_world(&world, entity))),
            }
        }
        fn get_network_ownership_auto(lua: &Lua, this: ObjectRef) -> LuaResult<bool> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            Ok(BasePartMembers::fetch_members(&world, this.entity()).auto_network_ownership)
        }
        fn get_no_collision_constraints(lua: &Lua, this: ObjectRef) -> LuaResult<Vec<ObjectRef>> {
            lua_todo!()
        }
        fn get_touching_parts(lua: &Lua, this: ObjectRef) -> LuaResult<Vec<ObjectRef>> {
            lua_todo!()
        }
        fn get_velocity_at_position(lua: &Lua, this: ObjectRef, position: Vector3) -> LuaResult<Vector3> {
            lua_todo!()
        }
        fn intersect_async(lua: &Lua, this: ObjectRef, parts: Vec<ObjectRef>) -> LuaResult<ObjectRef> {
            lua_todo!()
        }
        fn is_grounded(lua: &Lua, this: ObjectRef) -> LuaResult<bool> {
            lua_todo!()
        }
        #[deprecated_alias="resize"]
        fn resize(lua: &Lua, this: ObjectRef, normal_id: NormalId, delta_amount: f64) -> LuaResult<bool> {
            lua_todo!()
        }
        fn set_network_owner(lua: &Lua, this: ObjectRef, player: ObjectRef) -> LuaResult<()> {
            lua_todo!()
        }
        fn set_network_ownership_auto(lua: &Lua, this: ObjectRef) -> LuaResult<()> {
            lua_todo!()
        }
        fn subtract_async(lua: &Lua, this: ObjectRef, parts: Vec<ObjectRef>) -> LuaResult<ObjectRef> {
            lua_todo!()
        }
        fn torque_to_angular_acceleration(lua: &Lua, this: ObjectRef, torque: Vector3, ang_velocity: Option<Vector3>) -> LuaResult<Vector3> {
            lua_todo!()
        }
        fn union_async(lua: &Lua, this: ObjectRef, parts: Vec<ObjectRef>) -> LuaResult<ObjectRef> {
            lua_todo!()
        }

    }
}
