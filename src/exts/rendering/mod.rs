use crate::{
    core::{
        SchedulerPhase,
        extension::{EngineExtension, EngineExtensionDistribution, EngineExtensionInitLevel},
    },
    exts::rendering::{
        meshes::{assign_hash_meshes, assign_meshes},
        objects::trigger_changed_just_enabled,
    },
    internal_prelude::*,
};
use bevy::{ecs::schedule::ScheduleCleanupPolicy, prelude::*};
use bevy_rblx_derive::register;

mod materials;
mod meshes;
mod objects;

use materials::assign_materials;

#[derive(Default, Clone, Copy, Debug)]
pub struct RenderingExtension;

#[derive(SystemSet, Hash, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RenderPhases {
    TriggerChanged,
    PrepareRender,
    ExecuteRender,
    PostRender,
}

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

    fn post_core_init(&self, world: &mut World) {
        world.schedule_scope(PostUpdate, |_, s| {
            s.add_systems(
                (
                    (trigger_changed_just_enabled,).in_set(RenderPhases::TriggerChanged),
                    (
                        assign_materials,
                        assign_hash_meshes,
                        assign_meshes.after(assign_hash_meshes),
                    )
                        .in_set(RenderPhases::PrepareRender),
                    // ().in_set(RenderPhases::ExecuteRender),
                    // ().in_set(RenderPhases::PostRender)
                )
                    .chain()
                    .after(SchedulerPhase::PreRender),
            );
        });
    }

    fn post_shutdown_hook(&self, world: &mut World) {
        world.schedule_scope(PostUpdate, |world, s| {
            s.remove_systems_in_set(
                RenderPhases::TriggerChanged,
                world,
                ScheduleCleanupPolicy::RemoveSetAndSystems,
            )
            .unwrap();
            s.remove_systems_in_set(
                RenderPhases::PrepareRender,
                world,
                ScheduleCleanupPolicy::RemoveSetAndSystems,
            )
            .unwrap();
            // s.remove_systems_in_set(set, world, policy)
        })
    }
}
