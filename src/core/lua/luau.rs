use crate::{
    core::{FAST_FLAGS, lua::{ThreadIdentityType, singleton::{LuaSingleton, init_singletons}}, object::{ObjectHeader, DisabledObject}},
    internal_prelude::*,
};

use bevy::{platform::collections::HashMap, prelude::*};
use bevy_rblx_derive::{fast_flag, register};
use mlua::{
    Compiler,
    ffi::{lua_pushthread, lua_xmove},
    prelude::*,
};

#[derive(Component, Debug)]
pub struct LuauContainer {
    pub lua: Lua,
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
        Self { lua }.internal_init()
    }
}

impl LuauContainer {
    fn internal_init(mut self) -> Self {
        init_singletons(&mut self.lua).unwrap();

        self.lua.enable_jit(FAST_FLAGS.fetch::<FFLuauForceJit>());

        let compiler = Compiler::new()
            .set_debug_level(FAST_FLAGS.fetch::<FFLuauDebugLevel>() as u8)
            .set_optimization_level(FAST_FLAGS.fetch::<FFLuauOptimization>() as u8)
            .set_type_info_level(FAST_FLAGS.fetch::<FFLuauGlobalTypeInfoLevel>() as u8);
        self.lua.set_compiler(compiler);
        self.lua.gc_stop();

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
            let t = parent_lua
                .exec_raw::<LuaThread>((), |l| {
                    lua_pushthread(child);
                    lua_xmove(child, l, 1);
                })
                .unwrap();
            ThreadIdentity::set_thread(parent_lua, t, ThreadIdentity::fetch(parent_lua));
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
struct ThreadIdentityTable {
    identities: HashMap<usize, ThreadIdentity>,
    scripts: HashMap<Entity, HashMap<usize, LuaThread>>,
}

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
            .identities
            .get(&(lua.current_thread().to_pointer() as usize))
            .copied()
            .unwrap_or_default()
    }
    #[inline]
    pub unsafe fn set(lua: &Lua, id: Self) {
        let thr = lua.current_thread();
        unsafe { Self::set_thread(lua, thr, id) };
    }
    pub unsafe fn set_thread(lua: &Lua, thr: LuaThread, id: Self) {
        let idx = thr.to_pointer() as usize;
        let mut table = lua.app_data_mut::<ThreadIdentityTable>().unwrap();
        if let Some(old_id) = table.identities.get(&idx).copied() {
            if let Some(e) = old_id.script {
                let s = table.scripts.get_mut(&e).unwrap();
                s.remove(&idx);
                if s.len() == 0 {
                    table.scripts.remove(&e);
                }
            }
        }
        table.identities.insert(idx, id);
        if let Some(e) = id.script {
            table.scripts.entry(e).or_default().insert(idx, thr);
        }
    }
    pub fn erase_thr(lua: &Lua, thr_ptr: usize) {
        let mut table = lua.app_data_mut::<ThreadIdentityTable>().unwrap();
        if let Some(old_id) = table.identities.remove(&thr_ptr) {
            if let Some(e) = old_id.script {
                let s = table.scripts.get_mut(&e).unwrap();
                s.remove(&thr_ptr);
                if s.len() == 0 {
                    table.scripts.remove(&e);
                }
            }
        }
    }
    pub fn get_thread(lua: &Lua, thr: &LuaThread) -> Self {
        let idx = thr.to_pointer() as usize;
        lua.app_data_ref::<ThreadIdentityTable>()
            .unwrap()
            .identities
            .get(&idx)
            .copied()
            .unwrap_or_default()
    }
    pub fn get_threads(lua: &Lua, script: Entity) -> Vec<LuaThread> {
        let table = lua.app_data_ref::<ThreadIdentityTable>().unwrap();
        if let Some(threads) = table.scripts.get(&script) {
            threads.values().cloned().collect()
        } else {
            Vec::new()
        }
    }
}

fast_flag!(FFLuauForceJit: bool = false);
fast_flag!(FFLuauDebugLevel: u64 = 1);
fast_flag!(FFLuauOptimization: u64 = 1);
fast_flag!(FFLuauGlobalTypeInfoLevel: bool = false);

#[derive(Clone, Copy, Component, Debug)]
pub struct ContainerProvenance(pub Entity);

pub fn create_provenance(
    containers: Query<(Entity, Has<ContainerProvenance>), Added<LuauContainer>>,
    mut commands: Commands,
) {
    for e in containers
        .iter()
        .filter_map(|(e, h)| if h { Some(e) } else { None })
    {
        commands.entity(e).insert(ContainerProvenance(e));
    }
}

pub fn assign_provenance(
    missing_provenance: Query<
        Entity,
        (
            With<ObjectHeader>,
            Without<ContainerProvenance>,
            With<Children>,
        ),
    >,
    has_provenance: Query<Entity, With<LuauContainer>>,
    ancestors: Query<&ChildOf>,

    mut commands: Commands,
) {
    for e in missing_provenance.iter() {
        for ancestor in ancestors.iter_ancestors(e) {
            if has_provenance.contains(ancestor) {
                commands.entity(e).insert(ContainerProvenance(e));
                break;
            }
        }
    }
}

pub fn erase_provenance(
    e: Query<Entity, (With<ContainerProvenance>, With<DisabledObject>)>,
    mut c: Commands,
) {
    for i in e {
        c.entity(i).remove::<ContainerProvenance>();
    }
}
