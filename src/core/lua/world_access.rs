use std::cell::{RefCell, RefMut, UnsafeCell};
use std::mem::{replace, swap, take};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::core::lua::singleton::LuaSingleton;
use crate::core::templates::Takeable;
use crate::{self as bevy_rblx};
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::{AppDataRef, AppDataRefMut, prelude::*};

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
}

#[derive(Resource, Debug)]
pub(in crate::core) struct PlaceholderWorld(Option<World>);

impl Default for PlaceholderWorld {
    fn default() -> Self {
        Self(Some(World::new()))
    }
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
    Desynchronized(
        #[expect(dead_code)] UnsafeCell<RefMut<'a, CommandQueue>>,
        Commands<'a, 'a>,
    ),
}

#[repr(transparent)]
pub struct WorldAccessCommands<'a>(InternalWorldAccessCommands<'a>);

enum InternalWorldReadOnlyAccess<'a> {
    Synchronized(core::cell::Ref<'a, World>),
    Desynchronized(Arc<World>),
}

pub struct WorldAccessSyncGuard<'a>(Lua, &'a mut World, Takeable<PlaceholderWorld>);

impl<'a> Drop for WorldAccessSyncGuard<'a> {
    fn drop(&mut self) {
        let real_world = WorldAccess::fetch(&self.0).clear_sync_access();
        self.2.0 = Some(replace(self.1, real_world));
        self.1.insert_resource(self.2.take());
    }
}

pub(crate) struct WorldAccessSwapGuard<'a>(Lua, &'a mut WorldAccess);

impl<'a> Drop for WorldAccessSwapGuard<'a> {
    fn drop(&mut self) {
        swap(&mut WorldAccess::fetch(&self.0).0, &mut self.1.0);
    }
}

pub(crate) struct WorldAccessSwapLuaGuard(Lua, Lua);

impl Drop for WorldAccessSwapLuaGuard {
    fn drop(&mut self) {
        swap(
            &mut WorldAccess::fetch(&self.0).0,
            &mut WorldAccess::fetch(&self.1).0,
        );
    }
}

pub(in crate::core) struct WorldAccessDesyncGuard<'a> {
    lua_instances: Vec<Lua>,
    real_world: &'a mut World,
    placeholder_world: &'a mut Option<World>,
    arc_world: Option<Arc<World>>,
}
impl<'a> WorldAccessDesyncGuard<'a> {
    pub(in crate::core) fn new(w: &'a mut World, placeholder: &'a mut Option<World>) -> Self {
        let unwrapped_ph = placeholder.take().unwrap();
        let real_world = Arc::new(replace(w, unwrapped_ph));
        WorldAccessDesyncGuard {
            lua_instances: Vec::new(),
            real_world: w,
            placeholder_world: placeholder,
            arc_world: Some(real_world),
        }
    }
    pub(in crate::core) fn insert_mut(&mut self, lua: &Lua) {
        self.lua_instances.push(lua.clone());
        WorldAccess::fetch(lua).insert_desync_access(self.arc_world.clone().unwrap());
    }
}

impl<'a> Drop for WorldAccessDesyncGuard<'a> {
    fn drop(&mut self) {
        let mut q = CommandQueue::default();
        for i in self.lua_instances.iter_mut() {
            q.append(&mut WorldAccess::fetch(i).clear_desync_access());
        }
        let placeholder = replace(
            self.real_world,
            Arc::try_unwrap(self.arc_world.take().unwrap())
                .expect("arc world was not released by all lua instances"),
        );
        q.apply(self.real_world);
        *self.placeholder_world = Some(placeholder);
    }
}
#[derive(Deref, DerefMut)]
pub(in crate::core) struct WorldAccessCreateGuard<'a> {
    #[deref]
    access: WorldAccess,
    real_world: &'a mut World,
    placeholder_world: Takeable<PlaceholderWorld>,
}

impl<'a> Drop for WorldAccessCreateGuard<'a> {
    fn drop(&mut self) {
        match replace(&mut self.access.0, InternalWorldAccess::None) {
            InternalWorldAccess::Synchronized { world } => {
                let world = world.into_inner();
                self.placeholder_world.0 = Some(replace(&mut self.real_world, world));
                self.real_world.insert_resource(self.placeholder_world.take())
            }
            _ => unreachable!("invalid world access"),
        }
    }
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
    #[must_use = "world sync access is automatically dropped once the guard gets dropped. to control this behavior you may drop it manually."]
    pub fn insert_sync_access<'w: 's, 's>(
        &'s mut self,
        world: &'w mut World,
        lua: &'s Lua,
    ) -> WorldAccessSyncGuard<'w> {
        let mut placeholder_world = world.remove_resource::<PlaceholderWorld>().expect("WorldAccess aliasing detected.");
        self.0 = InternalWorldAccess::Synchronized {
            world: RefCell::new(replace(world, placeholder_world.0.take().unwrap())),
        };
        WorldAccessSyncGuard(lua.clone(), world, Takeable::new(placeholder_world))
    }
    fn insert_desync_access(&mut self, w: Arc<World>) {
        self.0 = InternalWorldAccess::Desynchronized {
            commands: RefCell::new(CommandQueue::default()),
            read_only_world: w,
        }
    }
    fn clear_desync_access(&mut self) -> CommandQueue {
        match replace(&mut self.0, InternalWorldAccess::None) {
            InternalWorldAccess::None => unreachable!("no world access while trying to clear it"),
            InternalWorldAccess::Desynchronized { mut commands, .. } => take(commands.get_mut()),
            _ => unreachable!("invalid world access"),
        }
    }
    fn clear_sync_access(&mut self) -> World {
        match replace(&mut self.0, InternalWorldAccess::None) {
            InternalWorldAccess::None => unreachable!("no world access while trying to clear it"),
            InternalWorldAccess::Synchronized { world, .. } => world.into_inner(),
            _ => unreachable!("invalid world access"),
        }
    }
    #[must_use = "world access is automatically dropped once the guard gets dropped. to control this behavior you may drop it manually."]
    pub(in crate::core) fn create<'a>(
        w: &'a mut World
    ) -> WorldAccessCreateGuard<'a> {
        let mut placeholder = w.remove_resource::<PlaceholderWorld>().expect("WorldAccess aliasing detected.");
        let world = replace(w, placeholder.0.take().unwrap());
        WorldAccessCreateGuard {
            access: WorldAccess(InternalWorldAccess::Synchronized {
                world: RefCell::new(world),
            }),
            real_world: w,
            placeholder_world: Takeable::new(placeholder),
        }
    }
    #[must_use = "world access is automatically dropped once the guard gets dropped. to control this behavior you may drop it manually."]
    pub(crate) fn borrow_into<'outer: 'borrow, 'borrow>(
        &'outer mut self,
        lua: &'borrow Lua,
    ) -> WorldAccessSwapGuard<'borrow> {
        swap(&mut self.0, &mut WorldAccess::fetch(lua).0);
        WorldAccessSwapGuard(lua.clone(), self)
    }

    #[must_use = "world access is automatically dropped once the guard gets dropped. to control this behavior you may drop it manually."]
    pub(crate) fn borrow_into_lua<'outer: 'borrow, 'borrow>(
        outer_lua: &'outer Lua,
        lua: &'borrow Lua,
    ) -> WorldAccessSwapLuaGuard {
        swap(
            &mut WorldAccess::fetch(outer_lua).0,
            &mut WorldAccess::fetch(lua).0,
        );
        WorldAccessSwapLuaGuard(lua.clone(), outer_lua.clone())
    }
    pub fn access_synchronized<'a>(&'a mut self) -> LuaResult<&'a mut World> {
        match &mut self.0 {
            InternalWorldAccess::None => unreachable!("invalid world access"),
            InternalWorldAccess::Synchronized { world, .. } => Ok(world.get_mut()),
            InternalWorldAccess::Desynchronized { .. } => Err(LuaError::runtime(
                "call is not allowed from a desynchronized context",
            )),
        }
    }
    pub fn access_commands<'a>(&'a self) -> WorldAccessCommands<'a> {
        match &self.0 {
            InternalWorldAccess::None => unreachable!("invalid world access"),
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
        }
    }
    pub fn access_read_only<'a>(&'a self) -> WorldReadOnlyAccess<'a> {
        match &self.0 {
            InternalWorldAccess::None => unreachable!("invalid world access"),
            InternalWorldAccess::Synchronized { world, .. } => {
                WorldReadOnlyAccess(InternalWorldReadOnlyAccess::Synchronized(world.borrow()))
            }
            InternalWorldAccess::Desynchronized {
                read_only_world, ..
            } => WorldReadOnlyAccess(InternalWorldReadOnlyAccess::Desynchronized(
                read_only_world.clone(),
            )),
        }
    }

    pub fn access_world_commands<'a>(&'a mut self) -> (&'a World, Commands<'a, 'a>) {
        match &mut self.0 {
            InternalWorldAccess::None => unreachable!("invalid world access"),
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
        }
    }

    pub fn is_desynchronized(&self) -> bool {
        match &self.0 {
            InternalWorldAccess::None | InternalWorldAccess::Synchronized { .. } => false,
            InternalWorldAccess::Desynchronized { .. } => true,
        }
    }

    pub fn assert_synchronized(&mut self) -> LuaResult<()> {
        match &mut self.0 {
            InternalWorldAccess::None => unreachable!("invalid world access"),
            InternalWorldAccess::Synchronized { .. } => Ok(()),
            InternalWorldAccess::Desynchronized { .. } => Err(LuaError::runtime(
                "call is not allowed from a desynchronized context",
            )),
        }
    }
    #[cfg(debug_assertions)]
    pub fn assert_valid(&self) {
        match &self.0 {
            InternalWorldAccess::None => unreachable!("invalid world access"),
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
                ref_mut.get_mut().flush();
            }
            InternalWorldAccessCommands::Desynchronized(_, _) => (),
        }
    }
}
