use bevy_rblx_derive::register_class;

use crate::{
    core::{WorldAccess, object::InstanceMembers},
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
        pub color: Color3,
        pub pivot_offset: CFrame,

        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            Vector3::from(world.get::<Transform>(this).unwrap().translation).into_lua(lua)
        }]
        virtual position: Vector3,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            Vector3::from(Vec3::from(world.get::<Transform>(this).unwrap().rotation.to_euler(EulerRot::YXZ))).into_lua(lua)
        }]
        virtual orientation: Vector3,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            Vector3::from(Vec3::from(world.get::<Transform>(this).unwrap().rotation.to_euler(EulerRot::XYZ))).into_lua(lua)
        }]
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
        pub reflectance: f64,
        pub transparency: f64
    }
    methods {}
}
