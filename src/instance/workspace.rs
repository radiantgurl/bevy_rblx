use bevy_rblx_derive::register_class;

use mlua::prelude::*;
// use bevy::prelude::*;
use crate::internal_prelude::*;

use crate::instance::ModelMembers;
use crate::core::ServiceMembers;

register_class! {
    abstract WorldRoot(Model)
    members {}
    methods {}
}

register_class! {
    priv Workspace(WorldRoot, Service)
    members {}
    methods {}
}