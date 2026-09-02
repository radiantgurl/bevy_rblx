use crate::{
    core::{
        SchedulerPhase,
        extension::{EngineExtensionDistribution, EngineExtensionInitLevel},
    },
    instance::UserInputServiceMembers,
    internal::EngineExtension,
    internal_prelude::*,
};
use bevy::{
    app::{App, PostUpdate},
    ecs::{
        system::{If, ParamSet, Single},
        world::World,
    },
};
use bevy_rblx_derive::register;

#[derive(Default)]
struct InputHandlingExt;

fn propagate_input_events(w: &mut World) {}

#[register]
impl EngineExtension for InputHandlingExt {
    fn id(&self) -> &'static str {
        "input_handling"
    }

    fn init_level(&self) -> EngineExtensionInitLevel {
        EngineExtensionInitLevel::Runtime
    }

    fn distribution(&self) -> EngineExtensionDistribution {
        EngineExtensionDistribution::Client
    }

    fn dyn_clone(&mut self, _app: &mut App) -> Box<dyn EngineExtension> {
        Box::new(Self)
    }

    fn description(&self) -> Option<&'static str> {
        Some("Input handling via UserInputService")
    }

    fn dynamically_removable(&self) -> bool {
        true
    }

    fn runtime_init(&self, world: &mut World) {
        world.schedule_scope(PostUpdate, |w, s| {
            s.add_systems(propagate_input_events.in_set(SchedulerPhase::Input));
        });
    }
    fn post_shutdown_hook(&self, world: &mut World) {
        todo!()
    }
}
