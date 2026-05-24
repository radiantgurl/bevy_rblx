use bevy::{app::App, ecs::world::World};

pub enum EngineExtensionInitLevel {
    Engine,
    PostCore,
    Runtime
}
#[allow(unused_variables)]
pub trait EngineExtension {
    fn name(&self) -> &'static str;
    fn init_level(&self) -> EngineExtensionInitLevel;

    fn engine_init(&self, app: &mut App) {}
    fn post_core_init(&self, world: &mut World) {}
    fn runtime_init(&self, world: &mut World) {}
    fn pre_shutdown_hook(&self, world: &mut World) {}
    fn post_shutdown_hook(&self, world: &mut World) {}
}
#[derive(Clone, Copy, Debug)]
pub struct EngineExtensionHook(pub fn() -> Box<dyn EngineExtension>);

inventory::collect!(EngineExtensionHook);