use std::mem::take;

use bevy::{
    DefaultPlugins, MinimalPlugins,
    app::{App, AppLabel, Last, PreUpdate, Startup},
    camera::Camera2d,
    ecs::{
        entity::Entity, schedule::IntoScheduleConfigs, system::Local, world::{CommandQueue, World}
    }, time::Time,
};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};

use crate::{
    core::{
        LuauContainer, RefCountedEntityCommandsExt as _, RefCountedPlugin,
        bind_close_system_runner,
        instance::NewInstanceEvent,
        luau::{assign_provenance, create_provenance},
    },
    userdata::instance_new,
};

pub struct Engine;

pub fn initialize(w: &mut World) {
    let mut container = LuauContainer::default();
    let mut commands = CommandQueue::default();
    let root_instance;
    {
        let unsafe_world = w.as_unsafe_world_cell();
        unsafe {
            container.enter_scope_sync(unsafe_world);
        }

        root_instance = instance_new(&container.lua, "DataModel".to_owned())
            .expect("datamodel was created")
            .entity();

        container.exit_scope(Some(&mut commands));
    }
    {
        let mut c = w.commands();
        c.entity(root_instance).protect();
    }

    w.entity_mut(root_instance).insert(container);
}

pub fn emit_event(startup_args: Local<Option<(Entity,)>>, w: &mut World) {
    
}

#[derive(AppLabel, Clone, Copy, Hash, Debug, Default, PartialEq, Eq)]
pub struct IntegratedServer;

impl Engine {
    fn additional(app: &mut App) {
        app.add_plugins(RefCountedPlugin);
        app.add_message::<NewInstanceEvent>();
        app.insert_resource(Time::from_hz(60.0));

        app.add_systems(
            Startup,
            (
                initialize,
                create_provenance,
                assign_provenance,
            ).chain(),
        );
        // app.add_systems(First, repl_receive);
        app.add_systems(PreUpdate, (
            create_provenance,
            assign_provenance
        ).chain());
        app.add_systems(Last, bind_close_system_runner);
    }

    pub fn headless() -> App {
        let mut app = App::new();

        app.add_plugins(MinimalPlugins);

        Self::additional(&mut app);

        app
    }

    pub fn default() -> App {
        let mut app = App::new();

        app.add_plugins(DefaultPlugins);
        app.add_plugins(EguiPlugin::default());
        if cfg!(debug_assertions) {
            app.add_plugins(WorldInspectorPlugin::default());
            app.world_mut().spawn(Camera2d);
        }

        Self::additional(&mut app);

        app
    }

    pub fn insert_integrated_server(client: &mut App) {
        let mut server = Self::headless();

        let server_subapp = take(&mut server.sub_apps_mut().main);
        client.insert_sub_app(IntegratedServer, server_subapp);
    }
}
