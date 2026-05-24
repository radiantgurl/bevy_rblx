use crate::internal_prelude::*;
use mlua::prelude::*;
use bevy_rblx_derive::register_class;
use crate::core::object::ServiceMembers;

register_class! {
    priv ReplicatedStorage(Service)
    members {}
    methods {}
}

register_class! {
    priv ReplicatedFirst(Service)
    members {}
    methods {}
}