use bevy_rblx_derive::register_class;

use mlua::prelude::*;
// use bevy::prelude::*;
use crate::internal_prelude::*;

use crate::core::object::ServiceMembers;
use crate::instance::ModelMembers;

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
