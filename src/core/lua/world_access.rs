use std::cell::{RefCell, RefMut, UnsafeCell};
use std::mem::{replace, take};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::core::lua::singleton::LuaSingleton;
use crate::{self as bevy_rblx};
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::{AppDataRef, AppDataRefMut, prelude::*};
use parking_lot::{Mutex, MutexGuard};

#[derive(Default)]
enum InternalWorldAccess {
    #[default]
    None,
    Synchronized {
        world: RefCell<World>,
    },
    Desynchronized {
        commands: RefCell<CommandQueue>,
        read_only_world: Arc<World>,
    },
    DesynchronizedWithQueue {
        commands: Arc<Mutex<CommandQueue>>,
        read_only_world: Arc<World>,
    },
}
impl std::fmt::Debug for InternalWorldAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Synchronized { .. } => write!(f, "Synchronized"),
            Self::Desynchronized { .. } => write!(f, "Desynchronized"),
            Self::DesynchronizedWithQueue { .. } => write!(f, "DesynchronizedWithQueue"),
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
enum InternalWorldAccessCommands<'a> {
    Synchronized(UnsafeCell<RefMut<'a, World>>, Commands<'a, 'a>),
    #[allow(dead_code)]
    Desynchronized(UnsafeCell<RefMut<'a, CommandQueue>>, Commands<'a, 'a>),
    #[allow(dead_code)]
    DesynchronizedWithQueue(UnsafeCell<MutexGuard<'a, CommandQueue>>, Commands<'a, 'a>),
}

#[repr(transparent)]
pub struct WorldAccessCommands<'a>(InternalWorldAccessCommands<'a>);

enum InternalWorldReadOnlyAccess<'a> {
    Synchronized(core::cell::Ref<'a, World>),
    Desynchronized(Arc<World>),
}

#[repr(transparent)]
pub struct WorldReadOnlyAccess<'a>(InternalWorldReadOnlyAccess<'a>);

impl<'a> Deref for WorldReadOnlyAccess<'a> {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        match &self.0 {
            InternalWorldReadOnlyAccess::Synchronized(reference) => reference.deref(),
            InternalWorldReadOnlyAccess::Desynchronized(world) => world,
        }
    }
}

impl WorldAccess {
    pub fn fetch<'a>(lua: &'a Lua) -> AppDataRefMut<'a, WorldAccess> {
        lua.app_data_mut().unwrap()
    }
    pub fn fetch_readonly<'a>(lua: &'a Lua) -> AppDataRef<'a, WorldAccess> {
        lua.app_data_ref().unwrap()
    }
    pub(crate) unsafe fn insert_sync_access(&mut self, w: &mut World) {
        self.0 = InternalWorldAccess::Synchronized {
            world: RefCell::new(take(w)),
        };
    }
    pub(crate) unsafe fn insert_desync_access(&mut self, w: Arc<World>) {
        self.0 = InternalWorldAccess::Desynchronized {
            commands: RefCell::new(CommandQueue::default()),
            read_only_world: w,
        }
    }
    /// Used internally before the lua ceases to exist
    pub(crate) unsafe fn insert_desync_custom_access(
        &mut self,
        w: Arc<World>,
        q: Arc<Mutex<CommandQueue>>,
    ) {
        self.0 = InternalWorldAccess::DesynchronizedWithQueue {
            commands: q,
            read_only_world: w,
        }
    }
    pub(crate) fn clear_desync_access(&mut self) -> Option<CommandQueue> {
        match replace(&mut self.0, InternalWorldAccess::None) {
            InternalWorldAccess::None => {
                panic!("Internal error: no world access while trying to clear it")
            }
            InternalWorldAccess::Desynchronized { mut commands, .. } => {
                Some(take(commands.get_mut()))
            }
            _ => panic!("Internal error: invalid world access"),
        }
    }
    pub(crate) fn clear_sync_access(&mut self, w: &mut World) {
        match replace(&mut self.0, InternalWorldAccess::None) {
            InternalWorldAccess::None => {
                panic!("Internal error: no world access while trying to clear it")
            }
            InternalWorldAccess::Synchronized { mut world, .. } => {
                std::mem::swap(world.get_mut(), w);
            }
            _ => panic!("Internal error: invalid world access"),
        }
    }
    pub fn access_synchronized<'a>(&'a mut self) -> LuaResult<&'a mut World> {
        match &mut self.0 {
            InternalWorldAccess::None => {
                unreachable!("Failed to access world instance, this should never happen.")
            }
            InternalWorldAccess::Synchronized { world, .. } => Ok(world.get_mut()),
            InternalWorldAccess::Desynchronized { .. }
            | InternalWorldAccess::DesynchronizedWithQueue { .. } => Err(LuaError::runtime(
                "cannot access underlying world in desynchronized phase",
            )),
        }
    }
    pub fn access_commands<'a>(&'a self) -> WorldAccessCommands<'a> {
        match &self.0 {
            InternalWorldAccess::None => {
                unreachable!("Failed to access world instance, this should never happen.")
            }
            InternalWorldAccess::Synchronized { world } => {
                let world_ref = UnsafeCell::new(world.borrow_mut());

                let commands = unsafe { (*world_ref.get()).commands() };

                WorldAccessCommands(InternalWorldAccessCommands::Synchronized(
                    world_ref, commands,
                ))
            }
            InternalWorldAccess::Desynchronized {
                commands,
                read_only_world,
            } => {
                let mut_ref = UnsafeCell::new(commands.borrow_mut());
                let commands = unsafe {
                    let queue = &mut **mut_ref.get();

                    Commands::new(queue, &**read_only_world)
                };

                WorldAccessCommands(InternalWorldAccessCommands::Desynchronized(
                    mut_ref, commands,
                ))
            }
            InternalWorldAccess::DesynchronizedWithQueue {
                commands,
                read_only_world,
            } => {
                let mut_ref = UnsafeCell::new(commands.lock());
                let commands = unsafe {
                    let queue = &mut **mut_ref.get();

                    Commands::new(queue, &**read_only_world)
                };

                WorldAccessCommands(InternalWorldAccessCommands::DesynchronizedWithQueue(
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
            }
            | InternalWorldAccess::DesynchronizedWithQueue {
                read_only_world, ..
            } => WorldReadOnlyAccess(InternalWorldReadOnlyAccess::Desynchronized(
                read_only_world.clone(),
            )),
        }
    }

    pub fn access_world_commands<'a>(&'a mut self) -> (&'a World, Commands<'a, 'a>) {
        match &mut self.0 {
            InternalWorldAccess::None => {
                unreachable!("Failed to access world instance, this should never happen.")
            }
            InternalWorldAccess::Synchronized { world, .. } => {
                let world_ref = world.get_mut();
                let world_ptr = &raw const *world_ref;

                unsafe { (&*world_ptr, world_ref.commands()) }
            }
            InternalWorldAccess::Desynchronized {
                commands,
                read_only_world,
            } => {
                let queue = commands.get_mut();
                (&**read_only_world, Commands::new(queue, read_only_world))
            }
            InternalWorldAccess::DesynchronizedWithQueue { .. } => todo!(),
        }
    }

    pub fn is_desynchronized(&self) -> bool {
        match &self.0 {
            InternalWorldAccess::None | InternalWorldAccess::Synchronized { .. } => false,
            InternalWorldAccess::Desynchronized { .. }
            | InternalWorldAccess::DesynchronizedWithQueue { .. } => true,
        }
    }

    pub fn assert_synchronized(&mut self) -> LuaResult<()> {
        match &mut self.0 {
            InternalWorldAccess::None => {
                unreachable!("Failed to access world instance, this should never happen.")
            }
            InternalWorldAccess::Synchronized { .. } => Ok(()),
            InternalWorldAccess::Desynchronized { .. }
            | InternalWorldAccess::DesynchronizedWithQueue { .. } => Err(LuaError::runtime(
                "cannot access underlying world in desynchronized phase",
            )),
        }
    }
    #[cfg(debug_assertions)]
    pub fn assert_valid(&self) {
        match &self.0 {
            InternalWorldAccess::None => panic!("Internal error: expected valid world access"),
            _ => (),
        }
    }
    #[cfg(not(debug_assertions))]
    #[inline(always)]
    pub fn assert_valid(&self) {}
}

impl<'a> Deref for WorldAccessCommands<'a> {
    type Target = Commands<'a, 'a>;

    fn deref(&self) -> &Self::Target {
        match &self.0 {
            InternalWorldAccessCommands::Synchronized(_, commands)
            | InternalWorldAccessCommands::Desynchronized(_, commands)
            | InternalWorldAccessCommands::DesynchronizedWithQueue(_, commands) => commands,
        }
    }
}

impl<'a> DerefMut for WorldAccessCommands<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.0 {
            InternalWorldAccessCommands::Synchronized(_, commands)
            | InternalWorldAccessCommands::Desynchronized(_, commands)
            | InternalWorldAccessCommands::DesynchronizedWithQueue(_, commands) => commands,
        }
    }
}

impl<'a> Drop for WorldAccessCommands<'a> {
    fn drop(&mut self) {
        match &mut self.0 {
            InternalWorldAccessCommands::Synchronized(ref_mut, _) => {
                ref_mut.get_mut().flush();
            }
            InternalWorldAccessCommands::Desynchronized(_, _) => (),
            InternalWorldAccessCommands::DesynchronizedWithQueue(_, _) => (),
        }
    }
}

pub(crate) enum WorldAccessDestructor {
    None,
    DestructPhase { commands: Arc<Mutex<CommandQueue>> },
}

#[register]
impl LuaSingleton for WorldAccessDestructor {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.set_app_data(Arc::new(Mutex::new(WorldAccessDestructor::None)));
        Ok(())
    }
}
