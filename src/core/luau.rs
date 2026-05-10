use crate::{core::FAST_FLAGS, internal_prelude::*};

use bevy::{
    ecs::world::{CommandQueue, unsafe_world_cell::UnsafeWorldCell},
    platform::collections::HashMap,
    prelude::*,
};
use bevy_rblx_derive::{fast_flag, register};
use mlua::{Compiler, prelude::*};

use crate::core::{LuaSingleton, ThreadIdentityType, WorldAccess, singleton::init_singletons};

#[derive(Component, Debug)]
pub struct LuauContainer {
    pub lua: Lua,
    in_scope: u32,
}

impl Clone for LuauContainer {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl Default for LuauContainer {
    fn default() -> Self {
        let lua = Lua::new_with(
            LuaStdLib::ALL_SAFE,
            LuaOptions::new().catch_rust_panics(false),
        )
        .unwrap();
        Self { lua, in_scope: 0 }.internal_init()
    }
}

impl LuauContainer {
    pub(super) unsafe fn enter_scope_sync(&mut self, world: UnsafeWorldCell) {
        debug_assert!(self.in_scope == 0);
        unsafe {
            self.lua
                .app_data_mut::<WorldAccess>()
                .unwrap()
                .insert_sync_access(world)
        }
        self.in_scope = 1;
    }
    pub(super) unsafe fn enter_scope_desync(&mut self, world: &World) {
        debug_assert!(self.in_scope == 0);
        unsafe {
            self.lua
                .app_data_mut::<WorldAccess>()
                .unwrap()
                .insert_desync_access(world)
        }
        self.in_scope = 2;
    }
    pub(super) fn exit_scope(&mut self, queue_ptr: Option<&mut CommandQueue>) {
        match self.in_scope {
            0 => panic!("expected to be called while still in scope"),
            1 => WorldAccess::fetch(&self.lua).clear_access(),
            2 => {
                let mut queue = WorldAccess::fetch(&self.lua)
                    .clear_desync_access()
                    .expect("still locked in desync");
                queue_ptr.unwrap().append(&mut queue);
            }
            _ => unreachable!(),
        };
        self.in_scope = 0;
    }
    fn internal_init(mut self) -> Self {
        init_singletons(&mut self.lua).unwrap();

        self.lua.enable_jit(FAST_FLAGS.fetch::<FFLuauForceJit>());

        let compiler = Compiler::new()
            .set_debug_level(FAST_FLAGS.fetch::<FFLuauDebugLevel>() as u8)
            .set_optimization_level(FAST_FLAGS.fetch::<FFLuauOptimization>() as u8)
            .set_type_info_level(FAST_FLAGS.fetch::<FFLuauGlobalTypeInfoLevel>() as u8);
        self.lua.set_compiler(compiler);

        unsafe {
            self.lua
                .exec_raw::<()>((), |l| {
                    let callbacks = mlua::ffi::lua_callbacks(l)
                        .as_mut()
                        .expect("lua_callbacks cant return null");

                    callbacks.userthread = Some(thread_create_delete_callback);
                })
                .unwrap();
        }

        self
    }
}

unsafe extern "C-unwind" fn thread_create_delete_callback(
    parent: *mut mlua::lua_State,
    child: *mut mlua::lua_State,
) {
    unsafe {
        if !parent.is_null() {
            let parent_lua = Lua::get_or_init_from_ptr(parent);
            ThreadIdentity::set_thread_raw(
                parent_lua,
                child as usize,
                ThreadIdentity::fetch(parent_lua),
            );
        } else {
            let lua = Lua::get_or_init_from_ptr(child);
            ThreadIdentity::erase_thr(lua, child as usize);
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct ThreadIdentity {
    pub identity: ThreadIdentityType,
    pub script: Option<Entity>,
}

#[derive(Default, Debug)]
struct ThreadIdentityTable(HashMap<usize, ThreadIdentity>);

#[register]
impl LuaSingleton for ThreadIdentityTable {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.set_app_data(ThreadIdentityTable::default());
        Ok(())
    }
}

impl ThreadIdentity {
    pub fn fetch(lua: &Lua) -> ThreadIdentity {
        lua.app_data_ref::<ThreadIdentityTable>()
            .unwrap()
            .0
            .get(&(lua.current_thread().to_pointer() as usize))
            .copied()
            .unwrap_or_default()
    }
    pub unsafe fn set(lua: &Lua, id: Self) {
        lua.app_data_mut::<ThreadIdentityTable>()
            .unwrap()
            .0
            .insert(lua.current_thread().to_pointer() as usize, id);
    }
    pub unsafe fn set_thread(lua: &Lua, thr: &LuaThread, id: Self) {
        lua.app_data_mut::<ThreadIdentityTable>()
            .unwrap()
            .0
            .insert(thr.to_pointer() as usize, id);
    }
    pub unsafe fn set_thread_raw(lua: &Lua, thr: usize, id: Self) {
        lua.app_data_mut::<ThreadIdentityTable>()
            .unwrap()
            .0
            .insert(thr, id);
    }
    pub fn erase_thr(lua: &Lua, thr_ptr: usize) {
        lua.app_data_mut::<ThreadIdentityTable>()
            .unwrap()
            .0
            .remove(&thr_ptr);
    }
}

fast_flag!(FFLuauForceJit: bool = false);
fast_flag!(FFLuauDebugLevel: u64 = 1);
fast_flag!(FFLuauOptimization: u64 = 1);
fast_flag!(FFLuauGlobalTypeInfoLevel: bool = false);
