use crate::{
    core::extension::{EngineExtension, EngineExtensionDistribution, EngineExtensionInitLevel},
    internal_prelude::*,
};
use bevy::prelude::*;
use bevy_rblx_derive::register;

#[derive(Default, Clone, Copy, Debug)]
pub struct RenderingExtension;

#[register]
impl EngineExtension for RenderingExtension {
    fn id(&self) -> &'static str {
        "rendering"
    }

    fn init_level(&self) -> EngineExtensionInitLevel {
        EngineExtensionInitLevel::EngineBuild
    }

    fn distribution(&self) -> EngineExtensionDistribution {
        EngineExtensionDistribution::Client
    }

    fn dyn_clone(&mut self, _app: &mut App) -> Box<dyn EngineExtension> {
        Box::new(Self)
    }

    fn name(&self) -> &'static str {
        "Rendering System"
    }

    fn description(&self) -> Option<&'static str> {
        Some("Rendering integration with Bevy Game Engine")
    }

    fn engine_build(&mut self, app: &mut App) {}

    fn post_shutdown_hook(&self, world: &mut World) {}
}
