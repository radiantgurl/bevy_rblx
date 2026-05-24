use std::{collections::BTreeMap, ops::DerefMut};

use crate::{internal_prelude::*, userdata::LuaSend};
use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::Deref,
};

use mlua::prelude::*;

pub struct LuaCallback {
    weak_lua: WeakLua,
    registry_key: LuaRegistryKey,
}

impl LuaCallback {
    pub fn new(lua: &Lua, func: LuaFunction) -> LuaResult<Self> {
        Ok(Self {
            weak_lua: lua.weak(),
            registry_key: lua.create_registry_value(func)?,
        })
    }
    pub fn try_clone(&self) -> LuaResult<Option<Self>> {
        if let Some(lua) = self.weak_lua.try_upgrade() {
            Ok(Some(Self {
                weak_lua: self.weak_lua.clone(),
                registry_key: lua
                    .create_registry_value(lua.registry_value::<LuaValue>(&self.registry_key)?)?,
            }))
        } else {
            Ok(None)
        }
    }
    pub fn as_function(&self) -> LuaResult<Option<(Lua, LuaFunction)>> {
        if let Some(lua) = self.weak_lua.try_upgrade() {
            let func = lua.registry_value::<LuaFunction>(&self.registry_key)?;
            Ok(Some((lua, func)))
        } else {
            Ok(None)
        }
    }
}

pub struct LuaCallbackTable<const ERASE_ON_CLONE: bool, T> {
    callbacks: HashMap<usize, (WeakLua, HashMap<i32, (LuaRegistryKey, T)>)>,
    ids: HashMap<usize, (usize, i32)>,
    id_counter: usize,
}

impl<const ERASE_ON_CLONE: bool, T> Default for LuaCallbackTable<ERASE_ON_CLONE, T> {
    fn default() -> Self {
        Self { callbacks: Default::default(), ids: Default::default(), id_counter: Default::default() }
    }
}

impl<const ERASE_ON_CLONE: bool, T> Clone for LuaCallbackTable<ERASE_ON_CLONE, T> 
    where T: Clone
{
    fn clone(&self) -> Self {
        if ERASE_ON_CLONE {
            Self::default()
        } else {
            let mut new_table = HashMap::default();
            for (ptr, (weak_lua, keys)) in self.callbacks.iter() {
                if let Some(lua) = weak_lua.try_upgrade() {
                    let mut v = HashMap::default();
                    for (key, (i, meta)) in keys {
                        v.insert(
                            *key,
                            (lua.create_registry_value(lua.registry_value::<LuaValue>(i).unwrap())
                                .unwrap(), meta.clone()),
                        );
                    }
                    new_table.insert(*ptr, (weak_lua.clone(), v));
                }
            }
            let new_id_table = self
                .ids
                .iter()
                .filter_map(|(id, (lua_weak_id, registry_id))| {
                    if new_table.contains_key(id) {
                        Some((*id, (*lua_weak_id, *registry_id)))
                    } else {
                        None
                    }
                })
                .collect();
            Self {
                callbacks: new_table,
                ids: new_id_table,
                id_counter: self.id_counter,
            }
        }
    }
}

impl<const ERASE_ON_CLONE: bool, T: Clone> LuaCallbackTable<ERASE_ON_CLONE, T> {
    pub fn insert(&mut self, lua: &Lua, function: LuaFunction, metadata: T) -> LuaResult<usize> {
        let ptr_hash = lua.to_pointer() as usize;
        let registry_key = lua.create_registry_value(function)?;
        let id = self.id_counter;
        self.ids.insert(id, (ptr_hash, registry_key.id()));
        self.id_counter += 1;
        self.callbacks
            .entry(ptr_hash)
            .or_insert_with(|| (lua.weak(), HashMap::default()))
            .1
            .insert(registry_key.id(), (registry_key, metadata));
        Ok(id)
    }
    pub fn try_insert(&mut self, callback: &LuaCallback, metadata: T) -> LuaResult<Option<usize>> {
        if let Some((lua, func)) = callback.as_function()? {
            Ok(Some(self.insert(&lua, func, metadata)?))
        } else {
            Ok(None)
        }
    }
    pub fn remove(&mut self, id: usize) {
        if let Some((lua_id, reg)) = self.ids.get(&id).copied() {
            self.callbacks.get_mut(&lua_id).unwrap().1.remove(&reg);
            self.ids.remove(&id);
        }
    }
    pub fn try_get_callback(&self, id: usize) -> LuaResult<Option<(LuaCallback, T)>> {
        if let Some((lua_id, reg)) = self.ids.get(&id).copied() {
            let (weak_lua, entries) = self.callbacks.get(&lua_id).unwrap();
            if let Some(lua) = weak_lua.try_upgrade() {
                let (key, meta) = entries.get(&reg).unwrap();
                Ok(Some((LuaCallback {
                    weak_lua: weak_lua.clone(),
                    registry_key: lua.create_registry_value(
                        lua.registry_value::<LuaValue>(key)?,
                    )?,
                }, meta.clone())))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    pub fn get_callbacks(&self) -> Vec<(Lua, Vec<(LuaFunction, T)>)> {
        self.callbacks
            .values()
            .filter_map(|(w, vals)| w.try_upgrade().map(|l| (l, vals)))
            .map(|(lua, keys)| {
                (
                    lua.clone(),
                    keys.values()
                        .map(|(reg, meta)| (lua.registry_value::<LuaFunction>(reg).unwrap(), meta.clone()))
                        .collect(),
                )
            })
            .collect()
    }
    pub fn garbage_collect(&mut self) {
        self.callbacks
            .retain(move |_, (w, _)| w.try_upgrade().is_some());
        self.ids
            .retain(|_, (lua_ptr, _)| self.callbacks.contains_key(lua_ptr));
    }
    pub fn contains(&self, id: usize) -> bool {
        self.ids.contains_key(&id)
    }
    pub fn get_expected_next_id(&self) -> usize {
        self.id_counter
    }
}

pub struct LuaPrioCallbackTable<const ERASE_ON_CLONE: bool, T> {
    callbacks: HashMap<usize, (WeakLua, HashMap<i32, (LuaRegistryKey, T)>)>,
    ids: HashMap<usize, (usize, i32, i64)>,
    id_counter: usize,
    priority_table: BTreeMap<i64, HashSet<usize>>,
}

impl<const ERASE_ON_CLONE: bool, T> Default for LuaPrioCallbackTable<ERASE_ON_CLONE, T> {
    fn default() -> Self {
        Self { callbacks: Default::default(), ids: Default::default(), id_counter: Default::default(), priority_table: Default::default() }
    }
}

impl<const ERASE_ON_CLONE: bool, T: Clone> Clone for LuaPrioCallbackTable<ERASE_ON_CLONE, T> {
    fn clone(&self) -> Self {
        if ERASE_ON_CLONE {
            Self::default()
        } else {
            let mut new_table = HashMap::default();
            for (ptr, (weak_lua, keys)) in self.callbacks.iter() {
                if let Some(lua) = weak_lua.try_upgrade() {
                    let mut v = HashMap::default();
                    for (key, (i, meta)) in keys {
                        v.insert(
                            *key,
                            (lua.create_registry_value(lua.registry_value::<LuaValue>(i).unwrap())
                                .unwrap(), meta.clone()),
                        );
                    }
                    new_table.insert(*ptr, (weak_lua.clone(), v));
                }
            }
            let new_id_table: HashMap<usize, (usize, i32, i64)> = self
                .ids
                .iter()
                .filter_map(|(id, (lua_weak_id, registry_id, prio))| {
                    if new_table.contains_key(id) {
                        Some((*id, (*lua_weak_id, *registry_id, *prio)))
                    } else {
                        None
                    }
                })
                .collect();
            let priority_table = self
                .priority_table
                .iter()
                .map(|(prio, values)| {
                    (
                        *prio,
                        values
                            .iter()
                            .filter_map(|x| {
                                if new_id_table.contains_key(x) {
                                    Some(*x)
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    )
                })
                .collect();
            Self {
                callbacks: new_table,
                ids: new_id_table,
                id_counter: self.id_counter,
                priority_table,
            }
        }
    }
}

impl<const ERASE_ON_CLONE: bool, T: Clone> LuaPrioCallbackTable<ERASE_ON_CLONE, T> {
    pub fn insert(&mut self, lua: &Lua, priority: i64, function: LuaFunction, metadata: T) -> LuaResult<usize> {
        let ptr_hash = lua.to_pointer() as usize;
        let registry_key = lua.create_registry_value(function)?;
        let id = self.id_counter;
        self.ids.insert(id, (ptr_hash, registry_key.id(), priority));
        self.id_counter += 1;
        self.callbacks
            .entry(ptr_hash)
            .or_insert_with(|| (lua.weak(), HashMap::default()))
            .1
            .insert(registry_key.id(), (registry_key, metadata));
        self.priority_table.entry(priority).or_default().insert(id);
        Ok(id)
    }
    pub fn try_insert(
        &mut self,
        priority: i64,
        callback: &LuaCallback,
        metadata: T
    ) -> LuaResult<Option<usize>> {
        if let Some((lua, func)) = callback.as_function()? {
            let id = self.insert(&lua, priority, func, metadata)?;
            self.priority_table.entry(priority).or_default().insert(id);
            Ok(Some(id))
        } else {
            Ok(None)
        }
    }
    pub fn remove(&mut self, id: usize) {
        if let Some((lua_id, reg, prio)) = self.ids.get(&id).copied() {
            self.callbacks.get_mut(&lua_id).unwrap().1.remove(&reg);
            self.ids.remove(&id);
            if {
                let ids = self.priority_table.get_mut(&prio).unwrap();
                ids.remove(&id);
                ids.len() == 0
            } {
                self.priority_table.remove(&prio);
            }
        }
    }
    pub fn try_get_callback(&self, id: usize) -> LuaResult<Option<(LuaCallback, T)>> {
        if let Some((lua_id, reg, _)) = self.ids.get(&id).copied() {
            let (weak_lua, entries) = self.callbacks.get(&lua_id).unwrap();
            if let Some(lua) = weak_lua.try_upgrade() {
                let (reg, metadata) = entries.get(&reg).unwrap();
                Ok(Some((LuaCallback {
                    weak_lua: weak_lua.clone(),
                    registry_key: lua.create_registry_value(
                        lua.registry_value::<LuaValue>(reg)?,
                    )?,
                }, metadata.clone())))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    pub fn get_callbacks(&self) -> Vec<(Lua, LuaFunction, T)> {
        let h: HashMap<usize, (Lua, HashMap<i32, (LuaFunction, T)>)> = self
            .callbacks
            .iter()
            .filter_map(|(id, (w, vals))| w.try_upgrade().map(|l| (id, l, vals)))
            .map(|(id, lua, keys)| {
                (
                    *id,
                    (
                        lua.clone(),
                        keys.iter()
                            .map(|(rid, (reg, metadata))| {
                                (*rid, (lua.registry_value::<LuaFunction>(reg).unwrap(), metadata.clone()))
                            })
                            .collect(),
                    ),
                )
            })
            .collect();
        self.priority_table
            .iter()
            .map(|(_, ids)| ids)
            .flatten()
            .map(|id| self.ids.get(id).unwrap())
            .filter_map(|(ptr_hash, reg, _)| {
                if let Some((lua, funcs)) = h.get(ptr_hash) {
                    let (f, metadata) = funcs.get(reg).unwrap().clone();
                    Some((lua.clone(), f, metadata))
                } else {
                    None
                }
            })
            .collect()
    }
    pub fn garbage_collect(&mut self) {
        self.callbacks
            .retain(move |_, (w, _)| w.try_upgrade().is_some());
        self.ids
            .retain(|_, (lua_ptr, _, _)| self.callbacks.contains_key(lua_ptr));

        self.priority_table.retain(|_, ids| {
            ids.retain(|id| self.ids.contains_key(id));
            ids.len() > 0
        });
    }
    pub fn contains(&self, id: usize) -> bool {
        self.ids.contains_key(&id)
    }
    pub fn get_expected_next_id(&self) -> usize {
        self.id_counter
    }
}

#[derive(Deref)]
pub struct LuaPrioCallbackTableCached<const ERASE_ON_CLONE: bool, T> {
    #[deref]
    callbacks: LuaPrioCallbackTable<ERASE_ON_CLONE, T>,
    cached: Option<Vec<(Lua, LuaFunction, T)>>,
}

impl<const ERASE_ON_CLONE: bool, T> Default for LuaPrioCallbackTableCached<ERASE_ON_CLONE, T> {
    fn default() -> Self {
        Self { callbacks: Default::default(), cached: None }
    }
}

impl<const ERASE_ON_CLONE: bool, T: Clone> Clone for LuaPrioCallbackTableCached<ERASE_ON_CLONE, T> {
    fn clone(&self) -> Self {
        Self {
            callbacks: self.callbacks.clone(),
            cached: None,
        }
    }
}

impl<const ERASE_ON_CLONE: bool, T> DerefMut for LuaPrioCallbackTableCached<ERASE_ON_CLONE, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.cached = None;
        &mut self.callbacks
    }
}

impl<const ERASE_ON_CLONE: bool, T: Clone> LuaPrioCallbackTableCached<ERASE_ON_CLONE, T> {
    pub fn get_callbacks_cached(&mut self) -> Vec<(Lua, LuaFunction, T)> {
        if let Some(r) = self.cached.as_ref() {
            r.clone()
        } else {
            let ret = self.get_callbacks();
            self.cached = Some(ret.clone());
            ret
        }
    }
}

impl<const ERASE_ON_CLONE: bool, T: LuaSend> LuaSend for LuaPrioCallbackTableCached<ERASE_ON_CLONE, T> {}
impl<const ERASE_ON_CLONE: bool, T: LuaSend> LuaSend for LuaPrioCallbackTable<ERASE_ON_CLONE, T> {}
impl<const ERASE_ON_CLONE: bool, T: LuaSend> LuaSend for LuaCallbackTable<ERASE_ON_CLONE, T> {}
