use bevy_rblx_derive::register_class;

use crate::{
    core::{WorldAccess, object::InstanceMembers},
    enums::Material,
    internal_prelude::*,
    userdata::{CFrame, Color3, Vector3},
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

        pub can_collide: bool,
        pub can_touch: bool,
        pub can_query: bool,
        pub center_of_mass: Vector3,
        pub collision_group: String,
        pub massless: bool,

        pub audio_can_collide: bool,

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
        pub color: Color3,
        #[default=Material::SmoothPlastic]
        pub material: Material
    }
    methods {}
}
