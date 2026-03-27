use std::sync::atomic::{
    AtomicU32,
    Ordering::{self, Relaxed},
};

use bevy::prelude::*;

#[derive(Component, Default)]
pub struct RefCounted {
    count: AtomicU32,
    protected: bool,
}

impl Clone for RefCounted {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl RefCounted {
    pub fn inc(&self) {
        self.count.fetch_add(1, Ordering::AcqRel);
    }
    pub fn dec(&self) {
        self.count.fetch_sub(1, Ordering::AcqRel);
    }
    pub fn should_delete(&self) -> bool {
        self.count.load(Ordering::Acquire) == 0
    }
    pub fn fetch_count(&self) -> u32 {
        self.count.load(Relaxed)
    }
    pub fn fetch_count_mut(&mut self) -> u32 {
        *self.count.get_mut()
    }
    pub fn new() -> Self {
        Self {
            count: AtomicU32::new(0),
            protected: false,
        }
    }
    pub fn new_protected() -> Self {
        Self {
            count: AtomicU32::new(1),
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
}

pub trait RefCountedEntityCommandsExt: Sized {
    fn inc_ref(&mut self) -> &mut Self;
    fn dec_ref(&mut self) -> &mut Self;
}

pub fn inc_ref_command(w: EntityWorldMut) -> Result<()> {
    w.get::<RefCounted>()
        .ok_or_else(|| "not a refcounted")?
        .inc();
    Ok(())
}
pub fn dec_ref_command(w: EntityWorldMut) -> Result<()> {
    w.get::<RefCounted>()
        .ok_or_else(|| "not a refcounted")?
        .dec();
    Ok(())
}

impl<'a> RefCountedEntityCommandsExt for EntityCommands<'a> {
    fn inc_ref(&mut self) -> &mut Self {
        self.queue(inc_ref_command)
    }

    fn dec_ref(&mut self) -> &mut Self {
        self.queue(dec_ref_command)
    }
}
