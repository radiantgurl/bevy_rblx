use std::{
    collections::HashMap,
    mem::take,
    process::exit,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use bevy::{
    DefaultPlugins, MinimalPlugins,
    app::{
        App, AppLabel, FixedUpdate, Last, PluginGroup as _, PostStartup, PostUpdate, PreUpdate, Startup, Update
    },
    camera::Camera2d,
    ecs::{
        error::BevyError,
        schedule::IntoScheduleConfigs,
        system::Local,
        world::{CommandQueue, World},
    },
    log::{Level, LogPlugin},
    tasks::{ComputeTaskPool, ParallelSlice},
    time::Time,
};
#[cfg(test)]
use bevy::{app::AppExit, ecs::message::MessageWriter};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
use clap::{ArgAction, value_parser};

use crate::{
    core::{
        FAST_FLAGS, FastFlagType, LoggedMessage, LuauContainer, RblxLogs, RefCountedEntityCommandsExt as _, RefCountedPlugin, TaskScheduler, WorldAccess, bind_close_system_runner, data_model::register_game_global, fastflags::FastFlagValue, input::start_input_handler, instance::NewInstanceEvent, luau::{assign_provenance, create_provenance, erase_provenance}, object::DisabledObject, run_service::RunServiceMembers
    },
    userdata::{RBXScriptSignal, instance_new},
};

pub struct Engine;

pub fn initialize(w: &mut World) {
    let container = LuauContainer::default();
    let root_instance;
    {
        unsafe {
            WorldAccess::fetch(&container.lua).insert_sync_access(w);
        }

        root_instance = instance_new(&container.lua, "DataModel".to_owned())
            .expect("datamodel was created")
            .entity();

        instance_new(&container.lua, "RunService".to_owned()).unwrap();
        instance_new(&container.lua, "CollectionService".to_owned()).unwrap();
        instance_new(&container.lua, "Workspace".to_owned()).unwrap();

        WorldAccess::fetch(&container.lua).clear_sync_access(w);
    }
    {
        let mut c = w.commands();
        c.entity(root_instance).protect();
    }

    w.entity_mut(root_instance).insert(container);
}

pub fn run_synchronized(world: &mut World) {
    let mut containers = world.query::<&LuauContainer>();

    let lua_cloned_iter = containers
        .iter(world)
        .map(|c| c.lua.clone())
        .collect::<Vec<_>>();
    for lua in lua_cloned_iter {
        unsafe { WorldAccess::fetch(&lua).insert_sync_access(world) };

        TaskScheduler::fetch(&lua).run(&lua, false, true, Duration::from_secs(0), None);
        lua.gc_collect().unwrap();

        WorldAccess::fetch(&lua).clear_sync_access(world);
    }
}
pub fn run_desynchronized(world: &mut World) {
    let mut containers_qs = world.query::<&LuauContainer>();
    let v = containers_qs
        .iter(world)
        .map(|x| x.lua.clone())
        .collect::<Vec<_>>();
    let arc = Arc::new(take(world));
    let queues = v.as_slice().par_chunk_map(
        ComputeTaskPool::get(),
        (v.len() / ComputeTaskPool::get().thread_num()).max(1),
        |_, containers| {
            let mut thread_queue = CommandQueue::default();
            for container in containers {
                unsafe {
                    let mut wa = WorldAccess::fetch(container);
                    wa.insert_desync_access(arc.clone());
                }

                TaskScheduler::fetch(&container).run(
                    &container,
                    true,
                    true,
                    Duration::from_secs(0),
                    None,
                );

                let mut wa = WorldAccess::fetch(container);
                wa.assert_valid();
                let mut queue = wa
                    .clear_desync_access()
                    .expect("desynced world has command queue");
                thread_queue.append(&mut queue);
            }
            thread_queue
        },
    );
    *world = Arc::try_unwrap(arc).expect("Failed to unwrap world");
    for mut i in queues {
        i.apply(world);
    }
}

pub fn dispatch_synchronized(world: &mut World) {
    let mut containers = world.query::<&LuauContainer>();

    let lua_cloned_iter = containers
        .iter(world)
        .map(|c| c.lua.clone())
        .collect::<Vec<_>>();
    for lua in lua_cloned_iter {
        unsafe { WorldAccess::fetch(&lua).insert_sync_access(world) };

        TaskScheduler::fetch(&lua).run(&lua, false, false, Duration::from_secs(0), None);

        WorldAccess::fetch(&lua).clear_sync_access(world);
    }
}
pub fn dispatch_desynchronized(world: &mut World) {
    let mut containers_qs = world.query::<&LuauContainer>();
    let v = containers_qs
        .iter(world)
        .map(|x| x.lua.clone())
        .collect::<Vec<_>>();
    let arc = Arc::new(take(world));
    let queues = v.as_slice().par_chunk_map(
        ComputeTaskPool::get(),
        (v.len() / ComputeTaskPool::get().thread_num()).max(1),
        |_, containers| {
            let mut thread_queue = CommandQueue::default();
            for container in containers {
                unsafe {
                    let mut wa = WorldAccess::fetch(container);
                    wa.insert_desync_access(arc.clone());
                }

                TaskScheduler::fetch(&container).run(
                    &container,
                    true,
                    false,
                    Duration::from_secs(0),
                    None,
                );

                let mut wa = WorldAccess::fetch(container);
                wa.assert_valid();
                let mut queue = wa
                    .clear_desync_access()
                    .expect("desynced world has command queue");
                thread_queue.append(&mut queue);
            }
            thread_queue
        },
    );
    *world = Arc::try_unwrap(arc).expect("Failed to unwrap world");
    for mut i in queues {
        i.apply(world);
    }
}

macro_rules! create_runservice_trigger {
    ($name: ident) => {
        concat_idents::concat_idents!(trigger_runservice_event = runservice_event, _, $name, {
            pub fn trigger_runservice_event(
                w: &mut World,
                mut cached_event: Local<Option<RBXScriptSignal>>,
            ) -> Result<(), BevyError> {
                if cached_event.is_none() {
                    let mut members_qs = w.query::<&RunServiceMembers>();
                    let members = members_qs.single(w).expect("run service is initialized");
                    *cached_event = Some(members.$name.clone());
                }

                let time = w.resource::<Time>().clone();

                let mut wa = WorldAccess::default();
                unsafe {
                    wa.insert_sync_access(w);
                }

                if stringify!($name) == "Stepped" {
                    cached_event.as_ref().unwrap().fire_outside_lua(
                        &mut wa,
                        false,
                        (time.elapsed_secs_f64(), time.delta_secs_f64()),
                    )?;
                } else {
                    cached_event.as_ref().unwrap().fire_outside_lua(
                        &mut wa,
                        false,
                        time.delta_secs_f64(),
                    )?;
                }
                wa.assert_valid();
                wa.clear_sync_access(w);
                Ok(())
            }
        });
    };
}

create_runservice_trigger!(heartbeat);
create_runservice_trigger!(post_simulation);
create_runservice_trigger!(pre_animation);
create_runservice_trigger!(pre_render);
create_runservice_trigger!(pre_simulation);
create_runservice_trigger!(stepped);

#[derive(AppLabel, Clone, Copy, Hash, Debug, Default, PartialEq, Eq)]
pub struct IntegratedServer;

static DEBUG_FLAG: AtomicBool = AtomicBool::new(false);

impl Engine {
    fn additional(app: &mut App) {
        app.add_plugins(RefCountedPlugin);
        app.add_plugins(bevy_async_commands::BevyAsyncCommandsPlugin);
        app.add_message::<NewInstanceEvent>();
        app.add_message::<LoggedMessage>();
        app.world_mut()
            .register_disabling_component::<DisabledObject>();
        app.insert_resource(RblxLogs::default());
        app.insert_resource(Time::from_hz(60.0));

        app.add_systems(
            Startup,
            (
                initialize,
                register_game_global,
                create_provenance,
                assign_provenance,
            )
                .chain(),
        );
        // app.add_systems(First, repl_receive);
        app.add_systems(
            PreUpdate,
            (
                create_provenance,
                assign_provenance,
                runservice_event_pre_animation,
                dispatch_synchronized,
                dispatch_desynchronized,
                create_provenance,
                assign_provenance,
                runservice_event_pre_simulation,
                runservice_event_stepped,
                dispatch_synchronized,
                dispatch_desynchronized,
            )
                .chain(),
        );
        app.add_systems(
            FixedUpdate,
            (
                create_provenance,
                assign_provenance,
                // step here
            )
                .chain(),
        );
        app.add_systems(
            Update,
            (
                register_game_global,
                (
                    create_provenance,
                    assign_provenance,
                    runservice_event_post_simulation,
                    dispatch_synchronized,
                    dispatch_desynchronized,
                    create_provenance,
                    assign_provenance,
                    run_synchronized.after(register_game_global),
                    run_desynchronized,
                    create_provenance,
                    assign_provenance,
                    runservice_event_heartbeat,
                    dispatch_synchronized,
                    dispatch_desynchronized,
                )
                    .chain(),
            ),
        );
        app.add_systems(
            PostUpdate,
            (
                create_provenance,
                assign_provenance,
                runservice_event_pre_render,
                dispatch_synchronized,
                dispatch_desynchronized,
            )
                .chain(),
        );
        app.add_systems(Last, (bind_close_system_runner, erase_provenance));

        // if cfg!(debug_assertions) {
        //     app.add_plugins(LogDiagnosticsPlugin::default());
        // }
    }

    pub fn headless() -> App {
        let mut app = App::new();

        app.add_plugins(MinimalPlugins.build().add(LogPlugin {
            level: if DEBUG_FLAG.load(std::sync::atomic::Ordering::Relaxed) {
                Level::DEBUG
            } else {
                Level::INFO
            },
            ..Default::default()
        }));

        Self::additional(&mut app);

        app
    }

    pub fn default() -> App {
        let mut app = App::new();

        app.add_plugins(DefaultPlugins.set(LogPlugin {
            level: if DEBUG_FLAG.load(std::sync::atomic::Ordering::Relaxed) {
                Level::DEBUG
            } else {
                Level::INFO
            },
            ..Default::default()
        }));
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

    #[cfg(test)]
    fn generate_exit_after_60_frames(
        mut frame_count: Local<u32>,
        exit_after: Local<u32>,
        mut writer: MessageWriter<AppExit>,
    ) {
        if *frame_count == *exit_after {
            writer.write(AppExit::Success);
        }
        *frame_count += 1;
    }

    #[cfg(test)]
    pub fn test_mode(exit_after_frames: u32) -> App {
        use crate::core::{FAST_FLAGS, scheduler::FFTaskSchedulerDisableWatchdog};
        use bevy::{
            app::{Main, PluginGroup as _, RunMode, ScheduleRunnerPlugin},
            ecs::system::{LocalBuilder, ParamBuilder, SystemParamBuilder},
        };

        let mut app = App::new();
        FAST_FLAGS.store::<FFTaskSchedulerDisableWatchdog>(true);

        app.add_plugins(
            MinimalPlugins
                .set(ScheduleRunnerPlugin {
                    run_mode: RunMode::Loop { wait: None },
                })
                .build()
                .add(LogPlugin {
                    level: if cfg!(debug_assertions) {
                        Level::DEBUG
                    } else {
                        Level::INFO
                    },
                    ..Default::default()
                }),
        );
        let built_system = (
            LocalBuilder(0),
            LocalBuilder(exit_after_frames),
            ParamBuilder,
        )
            .build_state(app.world_mut())
            .build_system(Engine::generate_exit_after_60_frames);
        app.add_systems(Main, built_system);

        Self::additional(&mut app);

        app
    }
    #[inline]
    pub fn main() {
        let args = clap::command!()
            .color(clap::ColorChoice::Always)
            .arg(
                clap::Arg::new("headless")
                    .long("headless")
                    .long_help("Run the engine in server mode")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("fastflag")
                    .short('f')
                    .long("fastflag")
                    .help("Set a fast flag")
                    .value_names(["FLAG", "VALUE"])
                    .value_parser(value_parser!(String)),
            )
            .arg(
                clap::Arg::new("dryrun")
                    .short('n')
                    .long("dry-run")
                    .help("Initialize the app and exit after")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("debug")
                    .short('d')
                    .long("debug")
                    .help("Enable debug logging")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("devconsole")
                    .long("devconsole")
                    .long_help("Enables the Developer Console")
                    .action(ArgAction::SetTrue)
            )
            .get_matches();
        let mut app;
        if let Some(fastflags) = args.get_occurrences("fastflag") {
            let ff_types = FAST_FLAGS.names_and_types().collect::<HashMap<_, _>>();
            for mut occurence in fastflags {
                let flag_name: &String = occurence.next().unwrap();
                let flag_value: &String = occurence.next().unwrap();
                println!("{flag_name} {flag_value}");
                if let Some(ff_type) = ff_types.get(flag_name.as_str()) {
                    let parsed = match ff_type {
                        FastFlagType::String => FastFlagValue::String(flag_value.clone()),
                        FastFlagType::Boolean => {
                            if let Ok(b) = flag_value.parse::<bool>() {
                                FastFlagValue::Boolean(b)
                            } else {
                                println!("Failed to parse bool flag for {flag_name}");
                                exit(1);
                            }
                        }
                        FastFlagType::Int => {
                            if let Ok(i) = flag_value.parse::<i64>() {
                                FastFlagValue::Int(i)
                            } else {
                                println!("Failed to parse int flag for {flag_name}");
                                exit(1);
                            }
                        }
                        FastFlagType::Uint => {
                            if let Ok(i) = flag_value.parse::<u64>() {
                                FastFlagValue::Uint(i)
                            } else {
                                println!("Failed to parse uint flag for {flag_name}");
                                exit(1);
                            }
                        }
                        FastFlagType::Float => {
                            if let Ok(i) = flag_value.parse::<f64>() {
                                FastFlagValue::Float(i)
                            } else {
                                println!("Failed to parse float flag for {flag_name}");
                                exit(1);
                            }
                        }
                    };
                    FAST_FLAGS.store_dyn(flag_name, parsed);
                } else {
                    println!("Invalid flag name: {flag_name}");
                    exit(1);
                }
            }
        }
        DEBUG_FLAG.store(args.get_flag("debug"), std::sync::atomic::Ordering::Relaxed);
        if args.get_flag("headless") {
            app = Engine::headless();
        } else {
            app = Engine::default();
        }
        if args.get_flag("devconsole") {
            app.add_systems(PostStartup, start_input_handler);
        }


        if args.get_flag("dryrun") {
            println!("dry run, exiting the app");
            exit(0);
        }
        app.run();
    }

    #[cfg(test)]
    pub(super) fn assert_no_errors(r: &RblxLogs) {
        use crate::enums::MessageType;

        for (msg_type, msg, _) in r.messages.clone() {
            if msg_type == MessageType::MessageError {
                panic!("Encountered a lua error:\n{msg}")
            }
        }
    }
}
