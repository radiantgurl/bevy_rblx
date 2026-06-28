use std::ops::DerefMut;

use bevy::prelude::*;

use crate::{core::object::DisabledObject, exts::rendering::materials, instance::BasePartMembers};

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Reflect, SystemSet, Debug)]
enum PostPreRenderPipeline {
    TriggerChanged,
    PrepareRender,
}

fn trigger_changed_just_enabled(
    mut parts: Query<&mut BasePartMembers>,
    mut removed_components: RemovedComponents<DisabledObject>,
) {
    for e in removed_components.read() {
        if let Ok(mut p) = parts.get_mut(e) {
            p.deref_mut();
        }
    }
}
pub fn trigger_rendering_reload(mut parts: Query<&mut BasePartMembers>) {
    for mut part in parts {
        part.deref_mut();
    }
}
