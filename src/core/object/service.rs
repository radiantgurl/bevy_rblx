use bevy::ecs::{
    component::Component,
    entity::Entity,
    hierarchy::{ChildOf, Children},
    lifecycle::RemovedComponents,
    query::{Allow, Changed, Has, With},
    system::{Commands, Query},
};
use bevy_rblx_derive::register_class;
use mlua::prelude::*;

use crate::{
    core::{
        lua::WorldAccess,
        object::{DisabledObject, Instance, InstanceMembers, RootInstance},
    },
    internal::ObjectHeader,
    internal_prelude::*,
};

#[derive(Clone, Copy, Component, Default, Debug)]
pub struct DisablingService;

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

pub(in crate::core) fn auto_disable_objects(
    mut commands: Commands,

    is_disabling_service: Query<Entity, With<DisablingService>>,
    is_disabled: Query<Has<DisabledObject>, With<ObjectHeader>>,

    ancestors: Query<&ChildOf, (Allow<DisabledObject>, With<ObjectHeader>)>,

    changed: Query<Entity, (Allow<DisabledObject>, Changed<ChildOf>)>,
    mut removed: RemovedComponents<ChildOf>,
) {
    for e in changed {
        let is_disabled = if let Ok(b) = is_disabled.get(e) {
            b
        } else {
            continue;
        };
        let expects_disabled = ancestors
            .iter_ancestors(e)
            .any(|e| is_disabling_service.contains(e));
        if expects_disabled != is_disabled {
            if expects_disabled {
                commands
                    .entity(e)
                    .insert_recursive::<Children>(DisabledObject);
            } else {
                commands
                    .entity(e)
                    .remove_recursive::<Children, DisabledObject>();
            }
        }
    }
    for e in removed.read() {
        let is_disabled = if let Ok(b) = is_disabled.get(e) {
            b
        } else {
            continue;
        };
        if is_disabled {
            commands
                .entity(e)
                .remove_recursive::<Children, DisabledObject>();
        }
    }
}
