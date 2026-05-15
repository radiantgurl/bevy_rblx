use std::cell::{Cell, RefCell, RefMut};
use std::mem::{replace, take, transmute};
use std::ops::{Deref, DerefMut};

use crate::{self as bevy_rblx};
use bevy::ecs::world::CommandQueue;
use bevy::ecs::world::unsafe_world_cell::UnsafeWorldCell;
use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::{AppDataRef, AppDataRefMut, prelude::*};

use crate::core::LuaSingleton;

#[derive(Default)]
enum InternalWorldAccess {
    #[default]
    None,
    Synchronized {
        #[deprecated(note = "todo: replace with RefCell<World>")]
        world: RefCell<&'static mut World>,
        commands_accessed: Cell<bool>,
    },
    Desynchronized {
        commands: RefCell<CommandQueue>,
        read_only_world: &'static World,
    },
}

impl std::fmt::Debug for InternalWorldAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Synchronized { .. } => write!(f, "Synchronized"),
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
#[deprecated(note="todo: the way commands are being accessed is very unsafe and should be done in a different way")]
enum InternalWorldAccessCommands<'a> {
    Synchronized(std::cell::RefMut<'a, &'static mut World>, Commands<'a, 'a>),
    Desynchronized(Box<RefMut<'a, CommandQueue>>, Commands<'a, 'a>),
}

#[repr(transparent)]
pub struct WorldAccessCommands<'a>(InternalWorldAccessCommands<'a>);

enum InternalWorldReadOnlyAccess<'a> {
    Synchronized(core::cell::Ref<'a, &'static mut World>),
    Desynchronized(&'static World),
}

#[repr(transparent)]
pub struct WorldReadOnlyAccess<'a>(InternalWorldReadOnlyAccess<'a>);

impl<'a> Deref for WorldReadOnlyAccess<'a> {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        match &self.0 {
            InternalWorldReadOnlyAccess::Synchronized(reference) => reference.deref(),
            InternalWorldReadOnlyAccess::Desynchronized(world) => *world,
        }
    }
}

#[repr(transparent)]
pub struct WorldAccessRef<'a> {
    inner: AppDataRefMut<'a, WorldAccess>,
}

impl WorldAccess {
    pub fn fetch<'a>(lua: &'a Lua) -> AppDataRefMut<'a, WorldAccess> {
        lua.app_data_mut().unwrap()
    }
    pub fn fetch_readonly<'a>(lua: &'a Lua) -> AppDataRef<'a, WorldAccess> {
        lua.app_data_ref().unwrap()
    }
    pub(crate) unsafe fn insert_sync_access(&mut self, w: UnsafeWorldCell) {
        self.0 = InternalWorldAccess::Synchronized {
            world: RefCell::new(unsafe { transmute::<_, &'static mut World>(w.world_mut()) }),
            commands_accessed: Cell::new(false),
        };
    }
    pub(crate) unsafe fn insert_desync_access(&mut self, w: &World) {
        self.0 = InternalWorldAccess::Desynchronized {
            commands: RefCell::new(CommandQueue::default()),
            read_only_world: unsafe { transmute(w) },
        }
    }
    pub(crate) fn clear_desync_access(&mut self) -> Option<CommandQueue> {
        match replace(&mut self.0, InternalWorldAccess::None) {
            InternalWorldAccess::Desynchronized { mut commands, .. } => {
                Some(take(commands.get_mut()))
            }
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
            InternalWorldAccess::Synchronized { world, .. } => Ok(world.get_mut()),
            InternalWorldAccess::Desynchronized { .. } => Err(LuaError::runtime(
                "cannot access underlying world in desynchronized phase",
            )),
        }
    }
    pub fn access_commands<'a>(&'a self) -> WorldAccessCommands<'a> {
        match &self.0 {
            InternalWorldAccess::None => {
                unreachable!("Failed to access world instance, this should never happen.")
            }
            InternalWorldAccess::Synchronized {
                world,
                commands_accessed,
            } => {
                let mut world_ref = world.borrow_mut();
                let world_ptr = &raw mut **world_ref.deref_mut();

                let commands = unsafe { (*world_ptr).commands() };

                commands_accessed.set(true);

                WorldAccessCommands(InternalWorldAccessCommands::Synchronized(
                    world_ref, commands,
                ))
            }
            InternalWorldAccess::Desynchronized {
                commands,
                read_only_world,
            } => {
                let mut_ref = Box::new(commands.borrow_mut());
                let ptr = Box::into_raw(mut_ref);
                let mut_ref = unsafe { Box::from_raw(ptr) };
                let commands = unsafe {
                    let queue = (*ptr).deref_mut();

                    Commands::new(queue, *read_only_world)
                };

                WorldAccessCommands(InternalWorldAccessCommands::Desynchronized(
                    mut_ref, commands,
                ))
            }
        }
    }
    pub fn access_read_only<'a>(&'a self) -> WorldReadOnlyAccess<'a> {
        match &self.0 {
            InternalWorldAccess::None => {
                unreachable!("Failed to access world instance, this should never happen.")
            }
            InternalWorldAccess::Synchronized { world, .. } => {
                WorldReadOnlyAccess(InternalWorldReadOnlyAccess::Synchronized(world.borrow()))
            }
            InternalWorldAccess::Desynchronized {
                read_only_world, ..
            } => WorldReadOnlyAccess(InternalWorldReadOnlyAccess::Desynchronized(read_only_world)),
        }
    }

    pub fn access_world_commands<'a>(&'a mut self) -> (&'a World, Commands<'a, 'a>) {
        match &mut self.0 {
            InternalWorldAccess::None => {
                unreachable!("Failed to access world instance, this should never happen.")
            }
            InternalWorldAccess::Synchronized { world, .. } => {
                let world_ref = world.get_mut();
                let world_ptr = std::ptr::from_ref(*world_ref);

                unsafe { (&*world_ptr, world_ref.commands()) }
            }
            InternalWorldAccess::Desynchronized {
                commands,
                read_only_world,
            } => {
                let queue = commands.get_mut();
                (*read_only_world, Commands::new(queue, read_only_world))
            }
        }
    }

    pub fn is_desynchronized(&self) -> bool {
        match &self.0 {
            InternalWorldAccess::None | InternalWorldAccess::Synchronized { .. } => false,
            InternalWorldAccess::Desynchronized { .. } => true,
        }
    }

    pub unsafe fn unsafe_readonly_world_cell(&self) -> UnsafeWorldCell<'static> {
        match &self.0 {
            InternalWorldAccess::None => {
                unreachable!("Failed to access world instance, this should never happen.")
            }
            InternalWorldAccess::Synchronized { world, .. } => unsafe {
                transmute(world.borrow().as_unsafe_world_cell_readonly())
            },
            InternalWorldAccess::Desynchronized {
                read_only_world, ..
            } => unsafe { transmute(read_only_world.as_unsafe_world_cell_readonly()) },
        }
    }

    pub unsafe fn unsafe_world_cell(&mut self) -> LuaResult<UnsafeWorldCell<'static>> {
        Ok(unsafe { transmute(self.access_synchronized()?.as_unsafe_world_cell()) })
    }
}

impl<'a> Deref for WorldAccessCommands<'a> {
    type Target = Commands<'a, 'a>;

    fn deref(&self) -> &Self::Target {
        match &self.0 {
            InternalWorldAccessCommands::Synchronized(_, commands)
            | InternalWorldAccessCommands::Desynchronized(_, commands) => commands,
        }
    }
}

impl<'a> DerefMut for WorldAccessCommands<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.0 {
            InternalWorldAccessCommands::Synchronized(_, commands)
            | InternalWorldAccessCommands::Desynchronized(_, commands) => commands,
        }
    }
}

impl<'a> Drop for WorldAccessCommands<'a> {
    fn drop(&mut self) {
        match &mut self.0 {
            InternalWorldAccessCommands::Synchronized(ref_mut, _) => {
                ref_mut.flush();
            }
            InternalWorldAccessCommands::Desynchronized(_, _) => (),
        }
    }
}
