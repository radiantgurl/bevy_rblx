use bevy::{
    app::App,
    ecs::{error::BevyError, resource::Resource, world::World},
    platform::collections::HashMap,
};

#[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
#[repr(u8)]
pub enum EngineExtensionInitLevel {
    // NOTE: Other extensions with ExtLoader stage cannot see eachother!
    ExtLoader,
    EngineBuild,
    PostCore,
    Runtime,
}
#[allow(unused_variables)]
pub trait EngineExtension: 'static + Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str {
        self.id()
    }
    fn description(&self) -> Option<&'static str> {
        None
    }
    fn path(&self) -> Option<&'static str> {
        None
    }
    fn init_level(&self) -> EngineExtensionInitLevel;
    fn dynamically_removable(&self) -> bool {
        false
    }

    fn dyn_clone(&mut self, app: &mut App) -> Box<dyn EngineExtension>; // NOTE: This is only done when an integrated server is requested,

    fn ext_load(
        &mut self,
        exts: &mut HashMap<&'static str, Box<dyn EngineExtension>>,
        app: &mut App,
    ) {
    }
    fn engine_build(&mut self, app: &mut App) {}
    fn post_core_init(&self, world: &mut World) {}
    fn runtime_init(&self, world: &mut World) {}
    fn pre_shutdown_hook(&self, world: &mut World) {}
    fn post_shutdown_hook(&self, world: &mut World) {}
}
#[derive(Clone, Copy, Debug)]
pub struct EngineExtensionHook(pub fn() -> Box<dyn EngineExtension>);

#[derive(Resource)]
pub struct EngineExtensions(pub(super) HashMap<&'static str, Box<dyn EngineExtension>>);

pub fn add_extension(w: &mut World, ext: Box<dyn EngineExtension>) -> Result<(), BevyError> {
    if w.resource::<EngineExtensions>().0.contains_key(ext.id()) {
        return Err(format!(
            "cannot insert extension {} as it was already added previously",
            ext.id()
        )
        .into());
    }
    if ext.init_level() == EngineExtensionInitLevel::Runtime {
        ext.runtime_init(w);
        let mut exts = w.resource_mut::<EngineExtensions>();
        let id = ext.id();
        exts.0.insert(id, ext);
        Ok(())
    } else {
        Err(format!(
            "a restart would be required to add the {:?} extension",
            ext.id()
        )
        .into())
    }
}

pub fn remove_extension(w: &mut World, id: &str) -> Result<(), BevyError> {
    let mut exts = w.resource_mut::<EngineExtensions>();
    if exts.0.contains_key(id) {
        return Err(format!("cannot remove extension {id} as it isn't loaded").into());
    }
    if !exts.0.get(id).unwrap().dynamically_removable() {
        return Err(format!("a restart would be required to remove extension {id}").into());
    }
    let ext = exts.0.remove(id).unwrap();
    drop(exts);
    ext.pre_shutdown_hook(w);
    ext.post_shutdown_hook(w);
    Ok(())
}

inventory::collect!(EngineExtensionHook);

pub(super) fn ext_post_core_init(w: &mut World) {
    bevy::log::debug!(target: "bevy_rblx::EngineExtensions", "extension post core init");
    let exts = w.remove_resource::<EngineExtensions>().unwrap();
    for ext in exts.0.values() {
        ext.post_core_init(w);
    }
    w.insert_resource(exts);
}
pub(super) fn ext_runtime_init(w: &mut World) {
    bevy::log::debug!(target: "bevy_rblx::EngineExtensions", "extension runtime init");
    let exts = w.remove_resource::<EngineExtensions>().unwrap();
    for ext in exts.0.values() {
        bevy::log::trace!(target: "bevy_rblx::EngineExtensions::ext_runtime_init", "running hook {}", ext.id());
        ext.runtime_init(w);
    }
    w.insert_resource(exts);
}
pub(super) fn ext_pre_shutdown(w: &mut World) {
    bevy::log::debug!(target: "bevy_rblx::EngineExtensions", "extension pre shutdown");
    let exts = w.remove_resource::<EngineExtensions>().unwrap();
    for ext in exts.0.values() {
        bevy::log::trace!(target: "bevy_rblx::EngineExtensions::ext_pre_shutdown", "running hook {}", ext.id());
        ext.pre_shutdown_hook(w);
    }
    w.insert_resource(exts);
}

pub(super) fn ext_post_shutdown(w: &mut World) {
    bevy::log::debug!(target: "bevy_rblx::EngineExtensions", "extension post shutdown");
    let exts = w.remove_resource::<EngineExtensions>().unwrap();
    for ext in exts.0.values() {
        bevy::log::trace!(target: "bevy_rblx::EngineExtensions::ext_post_shutdown", "running hook {}", ext.id());
        ext.post_shutdown_hook(w);
    }
    w.insert_resource(exts);
}
