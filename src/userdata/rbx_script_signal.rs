use std::{
    ops::DerefMut,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{
    core::{FAST_FLAGS, TaskScheduler, ThreadIdentity, WorldAccess},
    userdata::LuaSend,
};
use crate::{enums::SignalBehavior, internal_prelude::*};
use bevy::{platform::collections::HashMap, prelude::*};
use bevy_rblx_derive::fast_flag;
use mlua::prelude::*;
use parking_lot::RwLock;

#[derive(Default)]
pub struct RBXScriptSignalSingle {
    dispatch: HashMap<usize, LuaFunction>,
    parallel_dispatch: HashMap<usize, LuaFunction>,
    identities: HashMap<usize, ThreadIdentity>,
    count: usize,
}

struct ContainerEntry {
    registry: LuaRegistryKey,
    interrupt_early: Weak<AtomicBool>,
    weak_lua: WeakLua,
}

impl std::fmt::Debug for ContainerEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerEntry")
            .field("registry", &self.registry)
            .field("interrupt_early", &self.interrupt_early)
            .field(
                "weak_lua",
                &self.weak_lua.try_upgrade().map(|x| x.to_pointer()),
            )
            .finish()
    }
}

#[derive(Default, Debug)]
pub struct RBXScriptSignal {
    container_tables: Arc<RwLock<HashMap<usize, ContainerEntry>>>,
}

impl Clone for RBXScriptSignal {
    fn clone(&self) -> Self {
        Self {
            container_tables: Default::default(),
        }
    }
}

#[derive(Clone, FromLua)]
pub struct RBXScriptConnection {
    signal: LuaValue,
    id: usize,
    pd: bool,
}

impl RBXScriptSignalSingle {
    pub fn connect(
        &mut self,
        this_userdata: LuaAnyUserData,
        lua: &Lua,
        func: LuaFunction,
    ) -> LuaResult<RBXScriptConnection> {
        let count = self.count;
        self.count += 1;
        self.dispatch.insert(count, func);
        self.identities.insert(count, ThreadIdentity::fetch(lua));
        Ok(RBXScriptConnection {
            signal: this_userdata.into_lua(lua)?,
            id: count,
            pd: false,
        })
    }
    #[allow(dead_code)]
    pub fn connect_parallel(
        &mut self,
        this_userdata: LuaAnyUserData,
        lua: &Lua,
        func: LuaFunction,
    ) -> LuaResult<RBXScriptConnection> {
        let count = self.count;
        self.count += 1;
        self.parallel_dispatch.insert(count, func);
        self.identities.insert(count, ThreadIdentity::fetch(lua));
        Ok(RBXScriptConnection {
            signal: this_userdata.into_lua(lua)?,
            id: count,
            pd: false,
        })
    }
    pub fn once(
        &mut self,
        this_userdata: LuaAnyUserData,
        lua: &Lua,
        func: LuaFunction,
    ) -> LuaResult<RBXScriptConnection> {
        let count = self.count;
        self.count += 1;
        let conn = RBXScriptConnection {
            signal: this_userdata.into_lua(lua)?,
            id: count,
            pd: false,
        };
        let conn_clone = conn.clone();
        self.dispatch.insert(
            count,
            lua.create_function(move |lua: &Lua, values: LuaMultiValue| {
                TaskScheduler::fetch(lua).spawn(lua, func.clone(), values)?;
                conn_clone
                    .clone()
                    .into_lua(lua)?
                    .as_userdata()
                    .unwrap()
                    .call_method::<()>("Disconnect", ())
            })
            .unwrap(),
        );
        self.identities.insert(count, ThreadIdentity::fetch(lua));
        Ok(conn)
    }
}

impl LuaUserData for RBXScriptSignalSingle {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "RBXScriptSignal");
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_function(
            "Connect",
            move |lua: &Lua,
                  (this_userdata, func): (LuaAnyUserData, LuaFunction)|
                  -> LuaResult<RBXScriptConnection> {
                let mut this: LuaUserDataRefMut<Self> = this_userdata.borrow_typed_mut()?;
                this.connect(this_userdata, lua, func)
            },
        );
        methods.add_function(
            "ConnectParallel",
            move |lua: &Lua,
                  (this_userdata, func): (LuaAnyUserData, LuaFunction)|
                  -> LuaResult<RBXScriptConnection> {
                let mut this: LuaUserDataRefMut<Self> = this_userdata.borrow_typed_mut()?;
                let count = this.count;
                this.count += 1;
                this.parallel_dispatch.insert(count, func);
                this.identities.insert(count, ThreadIdentity::fetch(lua));
                Ok(RBXScriptConnection {
                    signal: this_userdata.into_lua(lua)?,
                    id: count,
                    pd: true,
                })
            },
        );
        methods.add_function(
            "Once",
            move |lua: &Lua,
                  (this_userdata, func): (LuaAnyUserData, LuaFunction)|
                  -> LuaResult<RBXScriptConnection> {
                let mut this: LuaUserDataRefMut<Self> = this_userdata.borrow_typed_mut()?;
                this.once(this_userdata, lua, func)
            },
        );
        methods.add_async_function(
            "Wait",
            async move |lua: Lua,
                        (this_userdata,): (LuaAnyUserData,)|
                        -> LuaResult<LuaMultiValue> {
                let count;
                {
                    let mut this: LuaUserDataRefMut<Self> = this_userdata.borrow_typed_mut()?;
                    count = this.count;
                    this.count += 1;
                    let thr = lua.current_thread();
                    let this_userdata_clone = this_userdata.clone();
                    //let count_clone = cl
                    this.dispatch.insert(
                        count,
                        lua.create_function(move |lua: &Lua, values: LuaMultiValue| {
                            let mut this: LuaUserDataRefMut<Self> =
                                this_userdata_clone.borrow_typed_mut().unwrap();
                            this.dispatch.remove(&count);
                            TaskScheduler::fetch(lua).spawn(lua, thr.clone(), values)?;
                            Ok(())
                        })
                        .unwrap(),
                    );
                    this.identities.insert(count, ThreadIdentity::fetch(&lua));
                }
                let values = lua.yield_with::<LuaMultiValue>(()).await;
                values
            },
        );
    }
}

impl LuaUserData for RBXScriptConnection {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "RBXScriptConnection");
        fields.add_field_function_get("Connected", |_lua, this| -> LuaResult<bool> {
            let this: LuaUserDataRef<Self> = this.borrow_typed()?;
            let signal: LuaUserDataRef<RBXScriptSignalSingle> = this.signal.borrow_typed().unwrap();
            Ok(if this.pd {
                signal.parallel_dispatch.get(&this.id).is_some()
            } else {
                signal.dispatch.get(&this.id).is_some()
            })
        });
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Disconnect", move |_lua: &Lua, this, ()| -> LuaResult<()> {
            let mut signal: LuaUserDataRefMut<RBXScriptSignalSingle> =
                this.signal.borrow_typed_mut().unwrap();
            if this.pd {
                signal.parallel_dispatch.remove(&this.id);
            } else {
                signal.dispatch.remove(&this.id);
            }
            signal.identities.remove(&this.id);
            Ok(())
        });
    }
}

impl<'a> IntoLua for &'a RBXScriptSignal {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let id = lua.to_pointer() as usize;
        if let Some(x) = self.container_tables.read().get(&id) {
            lua.registry_value(&x.registry)
        } else {
            let task = TaskScheduler::fetch(lua);
            let single = RBXScriptSignalSingle::default();
            let registry_key = lua.create_registry_value(single)?;
            let value = lua.registry_value(&registry_key);
            self.container_tables.write().insert(
                id,
                ContainerEntry {
                    registry: registry_key,
                    interrupt_early: task.get_early_interrupt_flag(),
                    weak_lua: lua.weak(),
                },
            );
            value
        }
    }
}

impl RBXScriptSignal {
    pub fn reference(&self) -> Self {
        Self {
            container_tables: self.container_tables.clone(),
        }
    }
    fn fetch_funcs(
        lua: &Lua,
        registry: &LuaRegistryKey,
        is_desync: bool,
    ) -> LuaResult<Vec<(LuaFunction, ThreadIdentity)>> {
        let single = lua.registry_value::<LuaUserDataRef<RBXScriptSignalSingle>>(registry)?;
        let dispatch_ref = if is_desync {
            &single.parallel_dispatch
        } else {
            &single.dispatch
        };
        Ok(dispatch_ref
            .iter()
            .map(|(i, f)| (f.clone(), single.identities.get(i).unwrap().clone()))
            .collect())
    }
    pub fn fire_in_lua(
        &self,
        lua: &Lua,
        ancestry_fire: bool,
        values: impl IntoLuaMulti + Clone + LuaSend,
    ) -> LuaResult<()> {
        let id = lua.to_pointer() as usize;
        for (key, container) in self.container_tables.read().iter() {
            if *key == id {
                Self::fire_internal(lua, values.clone(), &container.registry, false)?
            } else {
                if let (Some(interrupt_early), Some(external_lua)) = &(
                    container.interrupt_early.upgrade(),
                    container.weak_lua.try_upgrade(),
                ) {
                    interrupt_early.store(true, Ordering::Relaxed);

                    // NOTE: Lua will give up its app data when its ready to be accessed (ReentrantMutex)
                    {
                        let mut internal_world_access = WorldAccess::fetch(lua);
                        let mut external_world_access = WorldAccess::fetch(external_lua);
                        internal_world_access.assert_valid();
                        std::mem::swap(
                            internal_world_access.deref_mut(),
                            external_world_access.deref_mut(),
                        );
                    }
                    let res = Self::fire_internal(
                        external_lua,
                        values,
                        &container.registry,
                        ancestry_fire,
                    );
                    {
                        let mut internal_world_access = WorldAccess::fetch(lua);
                        let mut external_world_access = WorldAccess::fetch(external_lua);
                        std::mem::swap(
                            internal_world_access.deref_mut(),
                            external_world_access.deref_mut(),
                        );
                        internal_world_access.assert_valid();
                    }
                    return res;
                }
            }
        }
        Ok(())
    }
    fn fire_internal(
        lua: &Lua,
        values: impl IntoLuaMulti + Clone + LuaSend,
        registry: &LuaRegistryKey,
        ancestry_fire: bool,
    ) -> LuaResult<()> {
        let task = TaskScheduler::fetch(lua);
        let is_desync = task.is_desynchronized();
        let signal_behavior = FAST_FLAGS.fetch::<FFSignalBehavior>();
        let funcs = Self::fetch_funcs(lua, registry, is_desync)?;
        if signal_behavior == SignalBehavior::AncestryDeferred as u64 && ancestry_fire
            || signal_behavior == SignalBehavior::Deferred as u64
        {
            for (f, id) in funcs {
                let thr = task.defer(lua, f, values.clone())?;
                unsafe { ThreadIdentity::set_thread(lua, &thr, id) };
            }
        } else {
            for (f, id) in funcs {
                let thr = lua.create_thread(f)?;
                unsafe { ThreadIdentity::set_thread(lua, &thr, id) };
                task.spawn(lua, thr, values.clone())?;
            }
        }
        let funcs = Self::fetch_funcs(lua, registry, !is_desync)?;
        for (f, id) in funcs {
            let thr = task.defer_custom_pd(lua, f, values.clone(), !is_desync)?;
            unsafe { ThreadIdentity::set_thread(lua, &thr, id) };
        }
        Ok(())
    }
    pub fn fire_outside_lua(
        &self,
        internal_world_access: &mut WorldAccess,
        ancestry_fire: bool,
        values: impl IntoLuaMulti + Clone + LuaSend,
    ) -> LuaResult<()> {
        for (_, container) in self.container_tables.read().iter() {
            if let (Some(interrupt_early), Some(external_lua)) = &(
                container.interrupt_early.upgrade(),
                container.weak_lua.try_upgrade(),
            ) {
                interrupt_early.store(true, Ordering::Relaxed);
                internal_world_access.assert_valid();
                // NOTE: Lua will give up its app data when its ready to be accessed (ReentrantMutex)
                {
                    let mut external_world_access = WorldAccess::fetch(external_lua);
                    std::mem::swap(internal_world_access, external_world_access.deref_mut());
                }
                let res =
                    Self::fire_internal(external_lua, values, &container.registry, ancestry_fire);
                {
                    let mut external_world_access = WorldAccess::fetch(external_lua);
                    std::mem::swap(internal_world_access, external_world_access.deref_mut());
                }
                internal_world_access.assert_valid();
                return res;
            }
        }
        Ok(())
    }

    pub fn connect(&self, lua: &Lua, func: LuaFunction) -> LuaResult<RBXScriptConnection> {
        let v = self.into_lua(lua)?;
        let u = v.as_userdata().expect("must_be_userdata");
        let mut single: LuaUserDataRefMut<RBXScriptSignalSingle> = u.borrow_typed_mut()?;
        single.connect(u.clone(), lua, func)
    }
}

// 0 = IMMEDIATE
// 1 = DEFERRED
// 2 = ANCESTRYDEFERRED
pub struct LuaSendRBXScriptConnection {
    conn: Option<LuaRegistryKey>,
    weak_lua: WeakLua,
}

impl RBXScriptConnection {
    pub fn into_sendable(&self, lua: &Lua) -> LuaResult<LuaSendRBXScriptConnection> {
        Ok(LuaSendRBXScriptConnection {
            conn: Some(lua.create_registry_value(self.clone())?),
            weak_lua: lua.weak(),
        })
    }
    pub fn disconnect(&self) -> LuaResult<()> {
        let mut ud: LuaUserDataRefMut<RBXScriptSignalSingle> = self
            .signal
            .as_userdata()
            .expect("signal is userdata")
            .borrow_typed_mut()?;

        if self.pd {
            ud.parallel_dispatch.remove(&self.id);
        } else {
            ud.dispatch.remove(&self.id);
        }
        ud.identities.remove(&self.id);
        Ok(())
    }
}

impl LuaSendRBXScriptConnection {
    pub fn disconnect(&self) -> LuaResult<()> {
        if self.conn.is_none() {
            return Ok(());
        }
        if let Some(lua) = self.weak_lua.try_upgrade() {
            lua.registry_value::<RBXScriptConnection>(self.conn.as_ref().unwrap())?
                .disconnect()
        } else {
            Ok(())
        }
    }
}

impl Clone for LuaSendRBXScriptConnection {
    fn clone(&self) -> Self {
        if let Some(c) = self.conn.as_ref() {
            if let Some(lua) = self.weak_lua.try_upgrade() {
                let new_key = lua
                    .create_registry_value(
                        lua.registry_value::<LuaValue>(c)
                            .expect("expected no error while cloning"),
                    )
                    .expect("expected no error while cloning");

                return Self {
                    conn: Some(new_key),
                    weak_lua: self.weak_lua.clone(),
                };
            }
        }
        Self {
            conn: None,
            weak_lua: self.weak_lua.clone(),
        }
    }
}
fast_flag!(FFSignalBehavior: u64 = SignalBehavior::collapse_default(SignalBehavior::Default) as u64);
