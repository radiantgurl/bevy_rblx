use std::ops::DerefMut;

use bevy::prelude::*;

use crate::{
    core::object::DisabledObject,
    instance::{BasePartMembers, PartMembers},
};

pub(super) fn trigger_changed_just_enabled(
    mut base_parts: Query<&mut BasePartMembers>,
    mut parts: Query<&mut PartMembers>,
    mut removed_components: RemovedComponents<DisabledObject>,
) {
    for e in removed_components.read() {
        if let Ok(mut p) = base_parts.get_mut(e) {
            p.deref_mut();
        }
        if let Ok(mut p) = parts.get_mut(e) {
            p.deref_mut();
        }
    }
}
pub fn trigger_rendering_reload(
    base_parts: Query<&mut BasePartMembers>,
    parts: Query<&mut PartMembers>,
) {
    for mut base_part in base_parts {
        base_part.deref_mut();
    }
    for mut part in parts {
        part.deref_mut();
    }
}
