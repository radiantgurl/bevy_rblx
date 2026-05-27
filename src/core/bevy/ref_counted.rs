#[cfg(debug_assertions)]
use crate::core::engine::VERBOSE_FLAG;
use crate::{
    core::{FAST_FLAGS, object::object::DisabledObject},
    internal_prelude::*,
};
use bevy::{ecs::system::QueryLens, prelude::*};
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
    held: AtomicU32,
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
            held: AtomicU32::new(0),
        }
    }
}

impl RefCountedGroup {
    pub unsafe fn inc(&mut self) -> u32 {
        *self.held.get_mut() += 1;
        self.inner.fetch_add(1, Ordering::AcqRel)
    }
    unsafe fn inc_multiple(&mut self, amount: u32) -> u32 {
        *self.held.get_mut() += 1;
        self.inner.fetch_add(amount, Ordering::AcqRel)
    }
    pub unsafe fn dec(&mut self) -> u32 {
        *self.held.get_mut() -= 1;
        self.inner.fetch_sub(1, Ordering::AcqRel)
    }
}

impl Drop for RefCountedGroup {
    fn drop(&mut self) {
        let refs = *self.held.get_mut();
        self.inner.fetch_sub(refs, Ordering::AcqRel);
    }
}

#[derive(Component, Default, Reflect)]
pub struct RefCounted {
    count: AtomicU32,
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
        let r = self.count.fetch_add(1, Ordering::Relaxed)+1;
        if let Some(x) = self.group.as_mut() {
            return x.inc();
        }
        r
    }
    pub unsafe fn dec(&mut self) -> u32 {
        let r = self.count.fetch_sub(1, Ordering::Relaxed)-1;
        if let Some(x) = self.group.as_mut() {
            return x.dec();
        }
        r
    }
    pub fn should_delete_mut(&mut self) -> bool {
        if self.group.is_some() {
            self.group.as_ref().unwrap().inner.load(Ordering::Acquire) == 0
        } else {
            *self.count.get_mut() == 0
        }
    }

    pub fn fetch_count(&self) -> u32 {
        if self.group.is_some() {
            self.group.as_ref().unwrap().inner.load(Relaxed)
        } else {
            self.count.load(Relaxed)
        }
    }
    pub fn fetch_count_mut(&mut self) -> u32 {
        if self.group.is_some() {
            self.group.as_ref().unwrap().inner.load(Ordering::Acquire)
        } else {
            *self.count.get_mut()
        }
    }

    pub fn new() -> Self {
        Self {
            count: AtomicU32::new(0),
            group: None,
            protected: false,
        }
    }
    pub fn new_protected() -> Self {
        Self {
            count: AtomicU32::new(1),
            group: None,
            protected: true,
        }
    }
    pub fn protect(&mut self) {
        if !self.protected {
            *self.count.get_mut() += 1;
            self.protected = true;
        }
    }
    pub unsafe fn unprotect(&mut self) {
        if self.protected {
            *self.count.get_mut() -= 1;
            self.protected = false;
        }
    }
    pub fn get_group(&self) -> Option<&RefCountedGroup> {
        self.group.as_ref()
    }
    pub unsafe fn set_group(&mut self, group: Option<RefCountedGroup>) {
        self.group = group;
        let c = *self.count.get_mut();
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
            match bt.status()  {
                BacktraceStatus::Captured => {
                    let bt_str = bt.to_string();
                    for i in bt_str.split('\n').skip(1).step_by(2).take(9) {
                        println!("{i}");
                    }
                },
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
            match bt.status()  {
                BacktraceStatus::Captured => {
                    let bt_str = bt.to_string();
                    for i in bt_str.split('\n').skip(1).step_by(2).take(9) {
                        println!("{i}");
                    }
                },
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
            bevy::log::trace!(target: "bevy_rblx::RefCounted", "deleting entity {e} with {} references ({:?} group refs)", r.count.load(Relaxed), r.group.as_ref().map(|x| x.inner.load(Ordering::Acquire)));
            commands.entity(e).detach_all_children().despawn();
        } else {
            bevy::log::trace!(target: "bevy_rblx::RefCounted", "{e} has {} references ({:?} group refs)", r.count.load(Relaxed), r.group.as_ref().map(|x| x.inner.load(Ordering::Acquire)));
        }
    }
}
// SAFETY: mut RefCounted assures no entity can be deleted during execution of the group system.
pub fn assign_refcounted_groups(
    changed_entities: Query<Entity, (Changed<ChildOf>, With<RefCounted>, Allow<DisabledObject>)>,
    mut removed_parents: RemovedComponents<ChildOf>,

    mut refs: Query<&mut RefCounted, Allow<DisabledObject>>,

    mut parent_hierarchy: Query<Ref<ChildOf>, (With<RefCounted>, Allow<DisabledObject>)>,
    children_hierarchy: Query<&Children, (With<RefCounted>, Allow<DisabledObject>)>,
) {
    let changed_entities = changed_entities.iter().collect::<Vec<_>>();
    for e in changed_entities {
        let parent = parent_hierarchy.get(e).ok().map(|x| x.0);
        if let Some(parent) = parent {
            if !parent_hierarchy.get(e).unwrap().is_added() {
                let group = match refs
                    .get(parent)
                    .expect("queries should match")
                    .get_group()
                    .cloned()
                {
                    Some(group) => group,
                    None => {
                        let g = RefCountedGroup::default();
                        unsafe { refs.get_mut(parent).unwrap().set_group(Some(g.clone())) };
                        g
                    }
                };
                for i in children_hierarchy.iter_descendants(e).chain(once(e)) {
                    unsafe { refs.get_mut(i).unwrap().set_group(Some(group.clone())) };
                }
            } else {
                let mut lens: QueryLens<&ChildOf, Allow<DisabledObject>> =
                    parent_hierarchy.transmute_lens_filtered();
                let parent_hierarchy = lens.query();
                match (
                    refs.get(e)
                        .expect("queries should match")
                        .get_group()
                        .cloned(),
                    refs.get(parent)
                        .expect("queries should match")
                        .get_group()
                        .cloned(),
                ) {
                    (None, None) => {
                        let group = RefCountedGroup::default();
                        let root_entity = parent_hierarchy.root_ancestor::<ChildOf>(e);

                        for i in children_hierarchy
                            .iter_descendants(root_entity)
                            .chain(once(root_entity))
                        {
                            unsafe { refs.get_mut(i).unwrap().set_group(Some(group.clone())) };
                        }
                    }
                    (Some(group), None) => {
                        let root_entity = parent_hierarchy.root_ancestor::<ChildOf>(e);

                        for i in children_hierarchy
                            .iter_descendants(root_entity)
                            .chain(once(root_entity))
                        {
                            unsafe { refs.get_mut(i).unwrap().set_group(Some(group.clone())) };
                        }
                    }
                    (None, Some(group)) => {
                        for i in children_hierarchy.iter_descendants(e).chain(once(e)) {
                            unsafe { refs.get_mut(i).unwrap().set_group(Some(group.clone())) };
                        }
                    }
                    (Some(g1), Some(g2)) => {
                        // get bigger one
                        let root_entity;
                        let group;
                        if g1 == g2 {
                            continue;
                        }
                        if Arc::strong_count(&g1.inner) > Arc::strong_count(&g2.inner) {
                            root_entity = parent_hierarchy.root_ancestor::<ChildOf>(parent);
                            group = g1;
                        } else {
                            root_entity = parent_hierarchy.root_ancestor::<ChildOf>(e);
                            group = g2;
                        }

                        for i in children_hierarchy
                            .iter_descendants(root_entity)
                            .chain(once(root_entity))
                        {
                            unsafe { refs.get_mut(i).unwrap().set_group(Some(group.clone())) };
                        }
                    }
                }
            }
        } else {
        }
    }
    for e in removed_parents.read() {
        if !refs.contains(e) {
            continue;
        }

        let group = RefCountedGroup::default();

        for i in children_hierarchy.iter_descendants(e).chain(once(e)) {
            unsafe { refs.get_mut(i).unwrap().set_group(Some(group.clone())) };
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
