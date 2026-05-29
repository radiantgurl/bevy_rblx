#[cfg(debug_assertions)]
use crate::core::engine::VERBOSE_FLAG;
use crate::{
    core::{FAST_FLAGS, object::object::DisabledObject},
    internal_prelude::*,
};
use bevy::{platform::collections::HashSet, prelude::*};
use std::{
    iter::once,
    sync::{
        Arc,
        atomic::{
            AtomicU32,
            Ordering::{self, Relaxed},
        },
    },
};

#[derive(Default, Reflect)]
pub struct RefCountedGroup {
    inner: Arc<AtomicU32>,
    held: u32,
}

impl PartialEq for RefCountedGroup {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Clone for RefCountedGroup {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            held: 0,
        }
    }
}

impl RefCountedGroup {
    pub unsafe fn inc(&mut self) -> u32 {
        self.held += 1;
        self.inner.fetch_add(1, Ordering::AcqRel)
    }
    unsafe fn inc_multiple(&mut self, amount: u32) -> u32 {
        self.held += amount;
        self.inner.fetch_add(amount, Ordering::AcqRel)
    }
    pub unsafe fn dec(&mut self) -> u32 {
        self.held -= 1;
        self.inner.fetch_sub(1, Ordering::AcqRel)
    }
}

impl Drop for RefCountedGroup {
    fn drop(&mut self) {
        self.inner.fetch_sub(self.held, Ordering::AcqRel);
    }
}

#[derive(Component, Default, Reflect)]
pub struct RefCounted {
    count: u32,
    group: Option<RefCountedGroup>,
    protected: bool,
}

impl Clone for RefCounted {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl RefCounted {
    pub unsafe fn inc(&mut self) -> u32 {
        let r = self.count + 1;
        self.count = r;
        if let Some(x) = self.group.as_mut() {
            return unsafe { x.inc() };
        }
        r
    }
    pub unsafe fn dec(&mut self) -> u32 {
        let r = self.count - 1;
        self.count = r;
        if let Some(x) = self.group.as_mut() {
            return unsafe { x.dec() };
        }
        r
    }
    pub fn should_delete_mut(&mut self) -> bool {
        if self.group.is_some() {
            self.group.as_ref().unwrap().inner.load(Ordering::Acquire) == 0
        } else {
            self.count == 0
        }
    }

    pub fn fetch_count(&self) -> u32 {
        if self.group.is_some() {
            self.group.as_ref().unwrap().inner.load(Relaxed)
        } else {
            self.count
        }
    }
    pub fn fetch_count_mut(&mut self) -> u32 {
        if self.group.is_some() {
            self.group.as_ref().unwrap().inner.load(Ordering::Acquire)
        } else {
            self.count
        }
    }

    pub fn new() -> Self {
        Self {
            count: 0,
            group: None,
            protected: false,
        }
    }
    pub fn new_protected() -> Self {
        Self {
            count: 1,
            group: None,
            protected: true,
        }
    }
    pub fn protect(&mut self) {
        if !self.protected {
            unsafe { self.inc() };
            self.protected = true;
        }
    }
    pub unsafe fn unprotect(&mut self) {
        if self.protected {
            unsafe { self.dec() };
            self.protected = false;
        }
    }
    pub fn get_group(&self) -> Option<&RefCountedGroup> {
        self.group.as_ref()
    }
    pub unsafe fn set_group(&mut self, group: Option<RefCountedGroup>) {
        self.group = group;
        let c = self.count;
        if let Some(x) = self.group.as_mut() {
            unsafe { x.inc_multiple(c) };
        }
    }
}

pub trait RefCountedEntityCommandsExt: Sized {
    unsafe fn inc_ref(&mut self) -> &mut Self;
    unsafe fn dec_ref(&mut self) -> &mut Self;
    fn protect(&mut self) -> &mut Self;
}

pub mod commands {
    use super::{EntityWorldMut, RefCounted, Result};
    pub fn inc_ref_command(mut w: EntityWorldMut) -> Result<()> {
        let new_count = unsafe {
            w.get_mut::<RefCounted>()
                .ok_or_else(|| "not a refcounted")?
                .inc()
        };
        bevy::log::trace!(target: "bevy_rblx::RefCounted", "incrementing ref for {}, new count: {new_count}", w.id());
        Ok(())
    }
    pub fn dec_ref_command(mut w: EntityWorldMut) -> Result<()> {
        let new_count = unsafe {
            w.get_mut::<RefCounted>()
                .ok_or_else(|| "not a refcounted")?
                .dec()
        };
        bevy::log::trace!(target: "bevy_rblx::RefCounted", "decrementing ref for {}, new count: {new_count}", w.id());
        Ok(())
    }
    pub fn protect_command(mut w: EntityWorldMut) -> Result<()> {
        w.get_mut::<RefCounted>()
            .ok_or_else(|| "not a refcounted")?
            .protect();
        Ok(())
    }
}

use bevy_rblx_derive::fast_flag;
use commands::*;

impl<'a> RefCountedEntityCommandsExt for EntityCommands<'a> {
    unsafe fn inc_ref(&mut self) -> &mut Self {
        #[cfg(debug_assertions)]
        if VERBOSE_FLAG.load(Ordering::Relaxed) >= 3 {
            use std::backtrace::{Backtrace, BacktraceStatus};
            let bt = Backtrace::capture();
            match bt.status() {
                BacktraceStatus::Captured => {
                    let bt_str = bt.to_string();
                    for i in bt_str
                        .split('\n')
                        .skip(1)
                        .step_by(2)
                        .skip(1)
                        .take(9)
                        .filter(|x| !x.contains("rustup") && !x.contains("mlua-"))
                    {
                        println!("{i}");
                    }
                }
                _ => (),
            }
        }
        self.queue(inc_ref_command)
    }
    unsafe fn dec_ref(&mut self) -> &mut Self {
        #[cfg(debug_assertions)]
        if VERBOSE_FLAG.load(Ordering::Relaxed) >= 3 {
            use std::backtrace::{Backtrace, BacktraceStatus};
            let bt = Backtrace::capture();
            match bt.status() {
                BacktraceStatus::Captured => {
                    let bt_str = bt.to_string();
                    for i in bt_str
                        .split('\n')
                        .skip(1)
                        .step_by(2)
                        .skip(1)
                        .take(9)
                        .filter(|x| !x.contains("rustup") && !x.contains("mlua-"))
                    {
                        println!("{i}");
                    }
                }
                _ => (),
            }
        }
        self.queue(dec_ref_command)
    }
    fn protect(&mut self) -> &mut Self {
        self.queue(protect_command)
    }
}
pub fn refcounted_check_dead(
    mut q: Query<(Entity, &mut RefCounted), (Changed<RefCounted>, Allow<DisabledObject>)>,
    mut commands: Commands,
) {
    if FAST_FLAGS.fetch::<FFDisableRefCountedGC>() {
        return;
    }
    for (e, mut r) in q.iter_mut() {
        if r.should_delete_mut() {
            debug_assert!(r.fetch_count_mut() == 0);
            bevy::log::trace!(target: "bevy_rblx::RefCounted", "deleting entity {e} with {} references ({:?} group refs)", r.count, r.group.as_ref().map(|x| x.inner.load(Ordering::Acquire)));
            commands.entity(e).detach_all_children().despawn();
        } else {
            bevy::log::trace!(target: "bevy_rblx::RefCounted", "{e} has {} references ({:?} group refs)", r.count, r.group.as_ref().map(|x| x.inner.load(Ordering::Acquire)));
        }
    }
}

// SAFETY: mut RefCounted assures no entity can be deleted during execution of the group system.
pub fn assign_refcounted_groups(
    changed_entities: Query<Entity, (Changed<ChildOf>, With<RefCounted>, Allow<DisabledObject>)>,
    mut removed_parents: RemovedComponents<ChildOf>,

    mut refs: Query<&mut RefCounted, Allow<DisabledObject>>,

    ancestors: Query<&ChildOf, (With<RefCounted>, Allow<DisabledObject>)>,
    descendants: Query<&Children, (With<RefCounted>, Allow<DisabledObject>)>,
) {
    let mut checked = HashSet::new();
    for hierarchy in removed_parents.read() {
        if refs.contains(hierarchy) && !changed_entities.contains(hierarchy) {
            let hierarchy_group = RefCountedGroup::default();
            for e in once(hierarchy).chain(descendants.iter_descendants(hierarchy)) {
                unsafe {
                    refs.get_mut(e)
                        .unwrap()
                        .set_group(Some(hierarchy_group.clone()))
                };
            }
        }
    }

    'next_hierarchy: for mut hierarchy in changed_entities.iter() {
        if checked.contains(&hierarchy) {
            continue 'next_hierarchy;
        }
        'next_parent: loop {
            let parent = if let Ok(p) = ancestors.get(hierarchy) {
                p.0
            } else {
                break 'next_parent; // no valid group
            };
            let r = if let Ok(r) = refs.get_mut(ancestors.get(hierarchy).unwrap().0) {
                r
            } else {
                break 'next_parent; // no valid group
            };
            let group = r.get_group().cloned();
            drop(r);

            if let Some(group) = group {
                for e in once(hierarchy).chain(descendants.iter_descendants(hierarchy)) {
                    checked.insert(e);
                    unsafe { refs.get_mut(e).unwrap().set_group(Some(group.clone())) };
                }
                continue 'next_hierarchy;
            } else {
                hierarchy = parent;
                continue 'next_parent; // try again for a group
            }
        }
        // no valid group was found by this point, create a new one
        let group = RefCountedGroup::default();
        for e in once(hierarchy).chain(descendants.iter_descendants(hierarchy)) {
            checked.insert(e);
            unsafe { refs.get_mut(e).unwrap().set_group(Some(group.clone())) };
        }
    }
}
#[derive(Clone, Copy, Default, Debug, Reflect)]
pub struct RefCountedPlugin;

impl Plugin for RefCountedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Last,
            (assign_refcounted_groups, refcounted_check_dead).chain(),
        );
    }
}

fast_flag!(FFDisableRefCountedGC: bool = false);
