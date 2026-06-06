#![feature(negative_impls)]
#![feature(auto_traits)]
// #![warn(missing_docs)]

pub mod core;
pub mod enums;
pub mod exts;
pub mod instance;
pub mod userdata;

#[doc(hidden)]
pub mod internal {
    pub use inventory;

    pub trait LuaAsRef: Sized {
        fn as_lua_ref(&self) -> &Lua;
    }

    impl LuaAsRef for Lua {
        fn as_lua_ref(&self) -> &Lua {
            self
        }
    }

    pub use crate::core::extension::{EngineExtension, EngineExtensionHook};
    pub use crate::core::fastflags::{FastFlagKeyInsert, FastFlagKeyInserter};
    pub use crate::core::lua::CachedLuaFunction;
    pub use crate::core::lua::singleton::SingletonRegisterFn;
    pub use crate::core::object::{
        OBJECT_VTABLES, ObjectHeader, ObjectVTable,
        object::{
            ObjectContext, ObjectMethodInfo, ObjectNewFn, ObjectPropertyInfo,
            ObjectVTableCreationPointer,
        },
    };
    pub use crate::core::{SecurityContext, ThreadIdentity, WorldAccess};

    pub use bevy::ecs::entity::Entity;
    pub use bevy::ecs::system::EntityCommands;

    pub use mlua::Error as LuaError;
    pub use mlua::MultiValue as LuaMultiValue;
    pub use mlua::Result as LuaResult;
    pub use mlua::{FromLua, IntoLua, Lua};

    pub use crate::userdata::{LuaFreeValue, LuaSend, ObjectRef};

    pub use bevy::ecs::component::Component;
    pub use bevy::reflect::Reflect;
    pub use static_assertions::assert_impl_all;
}

pub(crate) use core::internal_prelude;

use crate::core::Engine;

fn main() {
    Engine::main()
}

#[cfg(test)]
mod tests;
