use bevy_rblx_derive::register_class;
use mlua::prelude::*;

use crate::{internal_prelude::*, userdata::RBXScriptSignal};
use super::InstanceMembers;

register_class! {
    abstract ServiceProvider(Instance)
    members {
        #[read_only]
        pub close: RBXScriptSignal,
        #[read_only]
        pub service_added: RBXScriptSignal,
        #[read_only]
        pub service_removing: RBXScriptSignal
    }
    methods {}
}