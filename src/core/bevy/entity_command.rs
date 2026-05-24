use std::marker::PhantomData;

use bevy::prelude::*;

pub struct EntityCommandWrapper<T, V: 'static + Send>
where
    T: EntityCommand<V>,
{
    pub e: Entity,
    pub c: T,
    pub phantom: PhantomData<V>,
}
impl<T, V: 'static + Send> EntityCommandWrapper<T, V>
where
    T: EntityCommand<V>,
{
    pub fn new(c: T, e: Entity) -> Self {
        Self {
            e,
            c,
            phantom: PhantomData::default(),
        }
    }
}
impl<T, V: 'static + Send> Command for EntityCommandWrapper<T, V>
where
    T: EntityCommand<V>,
{
    fn apply(self, world: &mut World) -> () {
        let Self { e, c, .. } = self;
        c.apply(world.entity_mut(e));
    }
}
