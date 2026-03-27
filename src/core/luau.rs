use bevy::{ecs::world::CommandQueue, prelude::*};
use mlua::prelude::*;

use crate::core::{WorldAccess, singleton::init_singletons};

#[derive(Component, Debug)]
pub struct LuauContainer {
    pub lua: Lua,
    queue: CommandQueue,
    in_scope: u32,
}

impl Clone for LuauContainer {
    fn clone(&self) -> Self {
        Self::default()
    }
}

#[cfg(debug_assertions)]
impl Default for LuauContainer {
    fn default() -> Self {
        let lua = Lua::new_with(
            LuaStdLib::ALL_SAFE,
            LuaOptions::new().catch_rust_panics(false),
        )
        .unwrap();
        Self {
            lua,
            in_scope: 0,
            queue: CommandQueue::default(),
        }
        .internal_init()
    }
}

#[cfg(not(debug_assertions))]
impl Default for LuauContainer {
    fn default() -> Self {
        let lua = Lua::new_with(
            LuaStdLib::ALL_SAFE,
            LuaOptions::new().catch_rust_panics(true),
        )
        .unwrap();
        Self { lua, in_scope: 0 }.internal_init()
    }
}

impl LuauContainer {
    unsafe fn enter_scope_sync(&mut self, world: &mut World) {
        debug_assert!(self.in_scope == 0);
        unsafe {
            self.lua
                .app_data_mut::<WorldAccess>()
                .unwrap()
                .insert_sync_access(world)
        }
        self.in_scope = 1;
    }
    unsafe fn enter_scope_desync(&mut self, world: &World) {
        debug_assert!(self.in_scope == 0);
        unsafe {
            self.lua
                .app_data_mut::<WorldAccess>()
                .unwrap()
                .insert_desync_access(world)
        }
        self.in_scope = 2;
    }
    fn exit_scope(&mut self) {
        match self.in_scope {
            1 => WorldAccess::fetch(&self.lua).clear_access(),
            2 => {
                let queue = WorldAccess::fetch(&self.lua)
                    .clear_desync_access()
                    .expect("still locked in desync");
            }
            0 => panic!("expected to be called while still in scope"),
            _ => unreachable!(),
        };
        self.in_scope = 0;
    }
    fn internal_init(mut self) -> Self {
        init_singletons(&mut self.lua).unwrap();
        self
    }
}
