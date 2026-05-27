use crate::{core::object::service::DisablingService, internal_prelude::*};
use mlua::prelude::*;
use bevy_rblx_derive::register_class;
use crate::core::object::ServiceMembers;

register_class! {
    #[require_components(DisablingService)]
    priv ReplicatedStorage(Service)
    members {}
    methods {}
}

register_class! {
    #[require_components(DisablingService)]
    priv ReplicatedFirst(Service)
    members {}
    methods {}
}