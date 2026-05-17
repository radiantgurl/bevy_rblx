#![feature(negative_impls)]
#![feature(auto_traits)]

pub mod core;
pub mod enums;
pub mod instance;
pub mod userdata;

#[doc(hidden)]
pub mod internal {
    pub use inventory;

    pub use crate::core::SecurityContext;
    pub use crate::core::ThreadIdentity;
    pub use crate::core::WorldAccess;
    pub use crate::core::fastflags::{FastFlagKeyInsert, FastFlagKeyInserter};
    pub use crate::core::object::{
        CachedLuaFunction, OBJECT_VTABLES, ObjectHeader, ObjectMethodInfo, ObjectNewFn,
        ObjectPropertyInfo,
    };
    pub use crate::core::{ObjectVTable, ObjectVTableCreationPointer};

    pub use bevy::ecs::entity::Entity;
    pub use bevy::ecs::system::EntityCommands;

    pub use mlua::Error as LuaError;
    pub use mlua::FromLua;
    pub use mlua::MultiValue as LuaMultiValue;
    pub use mlua::Result as LuaResult;

    pub use crate::userdata::LuaSend;
    pub use crate::userdata::ObjectRef;

    pub use static_assertions::assert_impl_all;
}

pub(crate) use core::internal_prelude;

use crate::core::Engine;

fn main() {
    let mut app = Engine::headless();

    app.run();
}

#[cfg(test)]
mod tests;
