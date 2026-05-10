use std::mem::take;

use bevy::{
    DefaultPlugins, MinimalPlugins,
    app::{App, AppLabel, Last, Startup},
    ecs::world::{CommandQueue, World},
};
use bevy_inspector_egui::bevy_egui::EguiPlugin;

use crate::{
    core::{
        LuauContainer, RefCountedPlugin, TaskScheduler, bind_close_system_runner,
        instance::NewInstanceEvent,
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
            let task = container
                .lua
                .app_data_ref::<TaskScheduler>()
                .expect("task scheduler exists");
            task.start_watchdog(None);
        }

        root_instance = instance_new(&container.lua, "DataModel".to_owned())
            .expect("datamodel was created")
            .entity();

        unsafe {
            {
                let task = container
                    .lua
                    .app_data_ref::<TaskScheduler>()
                    .expect("task scheduler exists");
                task.stop_watchdog();
            }
            container.exit_scope(Some(&mut commands));
        }
    }
    commands.apply(w);

    w.entity_mut(root_instance).insert(container);
}

#[derive(AppLabel, Clone, Copy, Hash, Debug, Default, PartialEq, Eq)]
pub struct IntegratedServer;

impl Engine {
    fn additional(app: &mut App) {
        app.add_plugins(RefCountedPlugin);
        app.add_message::<NewInstanceEvent>();

        app.add_systems(Startup, initialize);
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

        Self::additional(&mut app);

        app
    }

    pub fn insert_integrated_server(client: &mut App) {
        let mut server = Self::headless();

        let server_subapp = take(&mut server.sub_apps_mut().main);
        client.insert_sub_app(IntegratedServer, server_subapp);
    }
}
