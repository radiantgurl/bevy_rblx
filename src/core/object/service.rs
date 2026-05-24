use bevy::ecs::{entity::Entity, query::With, system::{Commands, Query}};
use bevy_rblx_derive::register_class;
use mlua::prelude::*;

use crate::{
    core::{
        lua::WorldAccess,
        object::{Instance, InstanceMembers, RootInstance},
    },
    internal_prelude::*,
};

register_class! {
    #[post_init=fn(lua: &Lua, this: Entity) -> LuaResult<()> {
        let root_instance = {
            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;
            {
                let mut members = InstanceMembers::fetch_members_mut(world, this);
                members.cloning_protected = true;
                members.destroy_protected = true;
                members.parent_protected = true;
            }
            let mut qs = world.query_filtered::<Entity, With<RootInstance>>();
            qs.single(world).expect("single game instance")
        };
        Instance::force_set_parent(lua, this, Some(root_instance))?;
        Ok(())
    }]
    priv abstract Service(Instance)
    members {}
    methods {}
}

pub(in crate::core) fn auto_disable_objects(mut commands: Commands, q: Query<Entity>) {

}