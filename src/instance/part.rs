use crate::internal_prelude::*;
use bevy_rblx_derive::register_class;
use mlua::prelude::*;

use super::BasePartMembers;
use crate::enums::PartType;

register_class! {
    Part(BasePart)
    members {
        #[default=PartType::Block]
        pub part_type: PartType
    }
    methods {}
}
