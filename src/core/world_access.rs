use std::mem::{replace, transmute};
use std::ops::Deref;

use crate::{self as bevy_rblx, core::ObjectHeader};
use bevy::ecs::system::lifetimeless::SCommands;
use bevy::ecs::world::CommandQueue;
use bevy::{
    ecs::{system::lifetimeless::Read, world::unsafe_world_cell::UnsafeWorldCell},
    prelude::*,
};
use bevy_rblx_derive::register;
use mlua::{AppDataRefMut, prelude::*};

use crate::core::LuaSingleton;

#[derive(Default)]
enum InternalWorldAccess {
    #[default]
    None,
    Synchronized(&'static mut World),
    Desynchronized {
        commands: CommandQueue,
        read_only_world: &'static World,
    },
}

impl std::fmt::Debug for InternalWorldAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Synchronized(_) => write!(f, "Synchronized"),
            Self::Desynchronized { .. } => write!(f, "Desynchronized"),
        }
    }
}

#[derive(Default)]
pub struct WorldAccess(InternalWorldAccess);
impl LuaUserData for WorldAccess {}

#[register]
impl LuaSingleton for WorldAccess {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.set_app_data(Self(InternalWorldAccess::None));
        Ok(())
    }
}

impl WorldAccess {
    pub fn fetch<'a>(lua: &'a Lua) -> AppDataRefMut<'a, WorldAccess> {
        lua.app_data_mut().unwrap()
    }
    pub(crate) unsafe fn insert_sync_access(&mut self, w: &mut World) {
        self.0 =
            InternalWorldAccess::Synchronized(unsafe { transmute::<_, &'static mut World>(w) });
    }
    pub(crate) unsafe fn insert_desync_access(&mut self, w: &World) {
        self.0 = InternalWorldAccess::Desynchronized {
            commands: CommandQueue::default(),
            read_only_world: unsafe { transmute(w) },
        }
    }
    pub(crate) fn clear_desync_access(&mut self) -> Option<CommandQueue> {
        match replace(&mut self.0, InternalWorldAccess::None) {
            InternalWorldAccess::Desynchronized { commands, .. } => Some(commands),
            _ => None,
        }
    }
    pub(crate) fn clear_access(&mut self) {
        self.0 = InternalWorldAccess::None;
    }
    pub fn access_synchronized<'a>(&'a mut self) -> LuaResult<&'a mut World> {
        match &mut self.0 {
            InternalWorldAccess::None => {
                unreachable!("Failed to access world instance, this should never happen.")
            }
            InternalWorldAccess::Synchronized(world) => Ok(&mut **world),
            InternalWorldAccess::Desynchronized { .. } => Err(LuaError::runtime(
                "cannot access underlying world in desynchronized phase",
            )),
        }
    }
    pub fn access_commands<'a>(&'a mut self) -> Commands<'a, 'a> {
        match &mut self.0 {
            InternalWorldAccess::None => {
                unreachable!("Failed to access world instance, this should never happen.")
            }
            InternalWorldAccess::Synchronized(world) => world.commands(),
            InternalWorldAccess::Desynchronized {
                commands,
                read_only_world,
            } => Commands::new(commands, read_only_world),
        }
    }
    pub fn access_read_only<'a>(&'a self) -> &'a World {
        match &self.0 {
            InternalWorldAccess::None => {
                unreachable!("Failed to access world instance, this should never happen.")
            }
            InternalWorldAccess::Synchronized(world) => &**world,
            InternalWorldAccess::Desynchronized {
                read_only_world, ..
            } => *read_only_world,
        }
    }
}

impl Deref for WorldAccess {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        self.access_read_only()
    }
}
