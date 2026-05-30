use std::{
    mem::{swap, take},
    process::exit,
    sync::{Arc, atomic::AtomicU8},
    time::{Duration, Instant},
};

#[cfg(test)]
use bevy::ecs::message::MessageWriter;
use bevy::{
    DefaultPlugins, MinimalPlugins,
    app::{
        App, AppExit, AppLabel, FixedUpdate, Last, PluginGroup as _, PostUpdate, PreUpdate,
        Startup, Update,
    },
    camera::Camera2d,
    ecs::{
        error::BevyError,
        message::{MessageReader, Messages},
        query::Allow,
        resource::Resource,
        schedule::{IntoScheduleConfigs, Schedule, SystemSet},
        system::{Commands, Local},
        world::{CommandQueue, World},
    },
    log::{Level, LogPlugin},
    platform::collections::HashMap,
    tasks::{ComputeTaskPool, ParallelSlice},
    time::Time,
};
use bevy_egui::EguiPrimaryContextPass;
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
use bevy_rblx_derive::fast_flag;
use clap::{ArgAction, ArgMatches, value_parser};
use parking_lot::Mutex;

use crate::{
    core::{
        FAST_FLAGS, FastFlagType, LoggedMessage, LuauContainer, RblxLogs, TaskScheduler,
        WorldAccess,
        bevy::ref_counted::RefCountedPlugin,
        extension::{
            EngineExtension, EngineExtensionDistribution, EngineExtensionInitLevel,
            EngineExtensions, ext_post_core_init, ext_post_shutdown, ext_pre_shutdown,
            ext_runtime_init,
        },
        fastflags::FastFlagValue,
        lua::{
            FFTaskSchedulerTimeSensitive, clock,
            luau::{assign_provenance, create_provenance, erase_provenance},
            world_access::WorldAccessDestructor,
        },
        object::{
            DisabledObject, NewInstanceEvent, RunServiceMembers,
            data_model::register_game_and_workspace_global, run_service::RunService,
            service::auto_disable_objects, service_provider::ServiceProviderMembers,
        },
    },
    enums::{CloseReason, SignalBehavior},
    internal_prelude::*,
    userdata::{FFSignalBehavior, RBXScriptSignal, instance_new},
};

#[derive(Resource, Clone, Copy, Hash, Default, Debug)]
pub struct Headless;
pub struct Engine;
#[derive(Resource, Clone, Copy)]
pub struct ShutdownReason(pub CloseReason);

fn take_world_local(l: &mut Option<World>, real_world: &mut World) -> World {
    let mut fake_world = l.take().unwrap();
    swap(&mut fake_world, real_world);
    fake_world
}
fn put_world_local(l: &mut Option<World>, mut fake_world: World, real_world: &mut World) {
    swap(&mut fake_world, real_world);
    *l = Some(fake_world);
}

pub(super) fn initialize(w: &mut World) {
    let container = LuauContainer::default();
    clock(); // initialize clock
    let root_instance;
    {
        unsafe {
            WorldAccess::fetch(&container.lua).insert_sync_access(w);
        }

        root_instance = instance_new(&container.lua, "DataModel".to_owned())
            .unwrap()
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

pub(super) fn run_synchronized(world: &mut World) {
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
pub(super) fn run_desynchronized(world: &mut World, mut l: Local<Option<World>>) {
    if l.is_none() {
        *l = Some(World::new());
    }
    let mut containers_qs = world.query::<&LuauContainer>();
    let v = containers_qs
        .iter(world)
        .map(|x| x.lua.clone())
        .collect::<Vec<_>>();
    let arc = Arc::new(take_world_local(&mut l, world));
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
    put_world_local(
        &mut l,
        Arc::into_inner(arc).expect("Failed to unwrap world"),
        world,
    );
    for mut i in queues {
        i.apply(world);
    }
}

pub(super) fn dispatch_synchronized(world: &mut World) {
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
pub(super) fn dispatch_desynchronized(world: &mut World, mut l: Local<Option<World>>) {
    if l.is_none() {
        *l = Some(World::new());
    }
    let mut containers_qs = world.query::<&LuauContainer>();
    let v = containers_qs
        .iter(world)
        .map(|x| x.lua.clone())
        .collect::<Vec<_>>();
    let arc = Arc::new(take_world_local(&mut l, world));
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
    put_world_local(
        &mut l,
        Arc::into_inner(arc).expect("Failed to unwrap world"),
        world,
    );
    for mut i in queues {
        i.apply(world);
    }
}

fn bind_close_system_runner(mut app_exit: MessageReader<AppExit>, mut c: Commands) {
    for _ in app_exit.read() {
        c.queue(Engine::shutdown)
    }
}

fn cleanup_instances(w: &mut World) {
    let mut containers_qs = w.query_filtered::<&mut LuauContainer, Allow<DisabledObject>>();
    let containers = containers_qs
        .iter_mut(w)
        .map(|mut x| take(&mut x.lua))
        .collect::<Vec<_>>();

    let arc_w = Arc::new(take(w));
    let arc_queue = Arc::new(Mutex::new(CommandQueue::default()));

    for lua in containers {
        unsafe {
            WorldAccess::fetch(&lua).insert_desync_custom_access(arc_w.clone(), arc_queue.clone());
        }
        *lua.app_data_ref::<Arc<Mutex<WorldAccessDestructor>>>()
            .unwrap()
            .lock() = WorldAccessDestructor::DestructPhase {
            commands: arc_queue.clone(),
        };
        lua.gc_restart();
        lua.set_globals(lua.create_table().unwrap()).unwrap();
        drop(lua);
    }

    *w = Arc::into_inner(arc_w).unwrap_or_else(|| {
        panic!("cleanup_instances failed during Engine::shutdown");
    });
    let mut queue = Arc::into_inner(arc_queue).unwrap();
    queue.get_mut().apply(w);
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
                    *cached_event = Some(members.$name.reference());
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

#[derive(Clone, Copy, Hash, Debug, PartialEq, Eq, PartialOrd, Ord, SystemSet)]
#[repr(u8)]
pub enum SchedulerPhase {
    ReplicationRecv,
    PreAnimation,
    HumanoidStep,
    PreSimulation,
    StepSimulation,
    PostSimulation,
    PreHeartbeat,
    Heartbeat,
    ReplicationSend,
    Input,
    PreRender,
}

pub static VERBOSE_FLAG: AtomicU8 = AtomicU8::new(0);

enum EnabledExts {
    Disable(Vec<String>),
    Enable(Vec<String>),
}
impl EnabledExts {
    fn should_be_enabled(&self, id: &'static str, default_enable: bool) -> bool {
        match self {
            EnabledExts::Disable(items) => !items.contains(&id.to_owned()) && default_enable,
            EnabledExts::Enable(items) => items.contains(&id.to_owned()),
        }
    }
}

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
                register_game_and_workspace_global,
                create_provenance,
                assign_provenance,
                ext_post_core_init,
                create_provenance,
                assign_provenance,
                ext_runtime_init,
                create_provenance,
                assign_provenance,
            )
                .chain(),
        );
        // app.add_systems(First, repl_receive);
        app.add_systems(
            PreUpdate,
            (
                (
                    auto_disable_objects,
                    runservice_event_pre_animation,
                    dispatch_synchronized,
                    dispatch_desynchronized,
                )
                    .chain()
                    .in_set(SchedulerPhase::PreAnimation)
                    .after(SchedulerPhase::ReplicationRecv),
                (
                    runservice_event_pre_simulation,
                    runservice_event_stepped,
                    dispatch_synchronized,
                    dispatch_desynchronized,
                )
                    .chain()
                    .in_set(SchedulerPhase::PreSimulation)
                    .after(SchedulerPhase::PreAnimation),
            ),
        );
        app.add_systems(
            FixedUpdate,
            (RunService::simulation_hook)
                .in_set(SchedulerPhase::StepSimulation)
                .after(SchedulerPhase::PreSimulation),
        );
        app.add_systems(
            Update,
            (
                register_game_and_workspace_global,
                (
                    (
                        runservice_event_post_simulation,
                        dispatch_synchronized,
                        dispatch_desynchronized,
                    )
                        .chain()
                        .in_set(SchedulerPhase::PostSimulation)
                        .after(SchedulerPhase::StepSimulation),
                    (
                        create_provenance,
                        assign_provenance,
                        run_synchronized.after(register_game_and_workspace_global),
                        run_desynchronized,
                    )
                        .chain()
                        .in_set(SchedulerPhase::PreHeartbeat)
                        .after(SchedulerPhase::PostSimulation),
                    (
                        create_provenance,
                        assign_provenance,
                        runservice_event_heartbeat,
                        dispatch_synchronized,
                        dispatch_desynchronized,
                    )
                        .chain()
                        .in_set(SchedulerPhase::Heartbeat)
                        .after(SchedulerPhase::PreHeartbeat),
                ),
            ),
        );
        if app.world().contains_resource::<Headless>() {
            app.add_systems(PostUpdate, (create_provenance, assign_provenance).chain());
        } else {
            app.add_systems(
                PostUpdate,
                (
                    create_provenance,
                    assign_provenance,
                    runservice_event_pre_render,
                    dispatch_synchronized,
                    dispatch_desynchronized,
                    RunService::render_hook,
                )
                    .chain()
                    .in_set(SchedulerPhase::PreRender)
                    .after(SchedulerPhase::Heartbeat)
                    .after(SchedulerPhase::ReplicationSend)
                    .after(SchedulerPhase::Input),
            );
        }
        app.add_systems(Last, (bind_close_system_runner, erase_provenance));
    }
    pub fn headless() -> App {
        let mut app = App::new();
        app.world_mut().insert_resource(Headless);

        app.add_plugins(MinimalPlugins.build().add(LogPlugin {
            level: match VERBOSE_FLAG.load(std::sync::atomic::Ordering::Relaxed) {
                0 => Level::INFO,
                1 => Level::DEBUG,
                _ => Level::TRACE,
            },
            ..Default::default()
        }));

        Self::additional(&mut app);

        app
    }
    pub fn default() -> App {
        let mut app = App::new();

        app.add_plugins(DefaultPlugins.set(LogPlugin {
            level: match VERBOSE_FLAG.load(std::sync::atomic::Ordering::Relaxed) {
                0 => Level::INFO,
                1 => Level::DEBUG,
                _ => Level::TRACE,
            },
            ..Default::default()
        }));
        app.add_plugins(EguiPlugin::default());
        if cfg!(debug_assertions) {
            app.add_plugins(WorldInspectorPlugin::default());
        } else {
            app.world_mut()
                .add_schedule(Schedule::new(EguiPrimaryContextPass));
        }
        app.world_mut().spawn(Camera2d);

        Self::additional(&mut app);

        app
    }

    fn integrated_server_extract(client: &mut World, server: &mut World) {
        if client.resource::<Messages<AppExit>>().len() != 0 {
            server.write_message(AppExit::Success);
        }
    }

    pub fn insert_integrated_server(client: &mut App) {
        let mut server = Self::headless();

        let mut server_subapp = take(&mut server.sub_apps_mut().main);

        server_subapp.set_extract(Self::integrated_server_extract);

        client.insert_sub_app(IntegratedServer, server_subapp);
    }
    #[cfg(test)]
    fn generate_exit_after_frame_count(
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
        use crate::core::{FAST_FLAGS, lua::FFTaskSchedulerDisableWatchdog};
        use bevy::{
            app::{Main, PluginGroup as _, RunMode, ScheduleRunnerPlugin},
            ecs::system::{LocalBuilder, ParamBuilder, SystemParamBuilder},
        };

        let mut app = App::new();
        app.world_mut().insert_resource(Headless);
        FAST_FLAGS.store::<FFTaskSchedulerDisableWatchdog>(true);

        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin {
            run_mode: RunMode::Loop { wait: None },
        }));
        let built_system = (
            LocalBuilder(0),
            LocalBuilder(exit_after_frames),
            ParamBuilder,
        )
            .build_state(app.world_mut())
            .build_system(Engine::generate_exit_after_frame_count);
        app.add_systems(Main, built_system);

        Self::additional(&mut app);

        Engine::load_extensions(&mut app, EnabledExts::Disable(Vec::default()));

        app
    }
    fn load_extensions(app: &mut App, enabled_exts: EnabledExts) {
        let distrib = if app.world().contains_resource::<Headless>() {
            EngineExtensionDistribution::Server
        } else {
            EngineExtensionDistribution::Client
        };
        let mut exts: HashMap<&'static str, Box<dyn EngineExtension>> = HashMap::new();
        let mut ext_loaders: HashMap<&'static str, Box<dyn EngineExtension>> = HashMap::new();
        for hook in inventory::iter::<crate::core::extension::EngineExtensionHook>() {
            let ext = hook.0();
            if !enabled_exts.should_be_enabled(ext.id(), ext.default_enabled())
                || !ext.distribution().matches(distrib)
            {
                continue;
            }
            if ext.init_level() == EngineExtensionInitLevel::ExtLoader {
                let id = ext.id();
                assert!(
                    ext_loaders.insert(id, ext).is_none(),
                    "duplicate extensions of {id} detected"
                );
            } else {
                let id = ext.id();
                assert!(
                    exts.insert(id, ext).is_none(),
                    "duplicate extensions of {id} detected"
                );
            }
        }
        for loader in ext_loaders.values_mut() {
            loader.ext_load(&mut exts, app);
        }

        exts.extend(ext_loaders.into_iter());
        exts.retain(|id, e| {
            enabled_exts.should_be_enabled(*id, true) && e.distribution().matches(distrib)
        });
        for ext in exts.values_mut() {
            ext.engine_build(app);
        }
        if app.get_sub_app(IntegratedServer).is_some() {
            let exts_clone = exts
                .iter_mut()
                .map(|(k, v)| (*k, v.dyn_clone(app)))
                .filter(|(_, e)| e.distribution().server())
                .collect::<HashMap<_, _>>();
            app.get_sub_app_mut(IntegratedServer)
                .unwrap()
                .world_mut()
                .insert_resource(EngineExtensions(exts_clone));
        }
        app.world_mut().insert_resource(EngineExtensions(exts));
    }
    fn parse_enabled_exts(args: &ArgMatches, enabled_exts: &mut EnabledExts) {
        if let Some(enabled) = args.get_one::<String>("enabled-exts") {
            *enabled_exts = EnabledExts::Enable(enabled.split(",").map(|x| x.to_owned()).collect());
        } else if let Some(disable) = args.get_one::<String>("disable-exts") {
            *enabled_exts =
                EnabledExts::Disable(disable.split(",").map(|x| x.to_owned()).collect());
        }
    }

    fn parse_fast_flags(args: &ArgMatches) {
        if args.get_flag("fastflags") {
            println!("NAME TYPE DEFAULT");
            for (name, ty) in FAST_FLAGS.names_and_types() {
                let v = FAST_FLAGS.fetch_dyn(name).unwrap();
                println!("{name} {ty} {v}");
            }
            exit(0);
        }
        if let Some(fastflags) = args.get_occurrences("fastflag") {
            let ff_types = FAST_FLAGS.names_and_types().collect::<HashMap<_, _>>();
            for mut occurence in fastflags {
                let flag_name: &String = occurence.next().unwrap();
                let flag_value: &String = occurence.next().unwrap();
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
    }

    #[inline(always)]
    pub fn main() {
        let args = clap::command!()
            .arg(
                clap::Arg::new("headless")
                    .help_heading("Runtime")
                    .long("headless")
                    .long_help("Run the engine in server mode")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("integrated-server")
                    .help_heading("Runtime")
                    .long("integrated-server")
                    .help("Adds an integrated server to a client")
                    .long_help("Adds an integrated server to the client build.\nIncompatible with --headless")
                    .conflicts_with("headless")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("fastflag")
                    .help_heading("Config")
                    .short('f')
                    .long("fastflag")
                    .help("Set a fast flag")
                    .value_names(["FLAG", "VALUE"])
                    .value_parser(value_parser!(String)),
            )
            .arg(
                clap::Arg::new("fastflags")
                    .help_heading("Config")
                    .long("fastflags")
                    .help("Show available fastflags")
                    .long_help("Show available fastflags and their corresponding types and defaults\nThis is printed as a table with 3 columns, the header and the values following it.")
                    .action(ArgAction::SetTrue)
            )
            .arg(
                clap::Arg::new("enabled-exts")
                    .help_heading("Config")
                    .long("enabled-exts")
                    .help("Only enable these extensions")
                    .long_help("Only enable these extensions.\nThe list of extensions is as a single, comma separated argument.")
                    .conflicts_with("disable-exts")
                    .value_names(["EXTENSIONS"])
                    .action(ArgAction::Set)
                    .value_parser(value_parser!(String)),
            )
            .arg(
                clap::Arg::new("disable-exts")
                    .help_heading("Config")
                    .long("disable-exts")
                    .help("Enable default and disable these extensions")
                    .long_help("Enable default and disable thes extensions\nThe list of extensions is as a single, comma separated argument.")
                    .conflicts_with("enabled-exts")
                    .value_names(["EXTENSIONS"])
                    .action(ArgAction::Set)
                    .value_parser(value_parser!(String)),
            )
            .arg(
                clap::Arg::new("dry-run")
                    .help_heading("Runtime")
                    .short('n')
                    .long("dry-run")
                    .help("Initialize the app and exit after")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("debug")
                    .short('v')
                    .long("verbose")
                    .alias("debug")
                    .help("Enable verbose logging")
                    .long_help("Enable verbose logging\nPassing this twice enables trace logging")
                    .action(ArgAction::Count),
            )
            .get_matches();
        let mut app;

        Engine::parse_fast_flags(&args);

        VERBOSE_FLAG.store(
            args.get_count("debug"),
            std::sync::atomic::Ordering::Relaxed,
        );

        if args.get_flag("headless") {
            app = Engine::headless();
        } else {
            app = Engine::default();
        }
        if args.get_flag("integrated-server") {
            Engine::insert_integrated_server(&mut app);
        }
        let mut enabled_exts = EnabledExts::Disable(Vec::new());
        Engine::parse_enabled_exts(&args, &mut enabled_exts);
        Engine::load_extensions(&mut app, enabled_exts);

        if args.get_flag("dry-run") {
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

    pub fn shutdown(w: &mut World) {
        ext_pre_shutdown(w);
        let close = w
            .query::<&ServiceProviderMembers>()
            .single(w)
            .expect("root instance exists while exiting app")
            .close
            .reference();
        let mut containers_qs = w.query::<&LuauContainer>();
        for container in containers_qs.iter(w) {
            TaskScheduler::fetch(&container.lua).prepare_for_shutdown();
        }
        let reason = w
            .get_resource::<ShutdownReason>()
            .copied()
            .map(|x| x.0)
            .unwrap_or(CloseReason::Unknown);
        let prev = FAST_FLAGS.fetch::<FFSignalBehavior>();
        {
            let mut wa = WorldAccess::default();
            unsafe {
                wa.insert_sync_access(w);
            }
            FAST_FLAGS.store::<FFSignalBehavior>(SignalBehavior::Deferred as u64);
            close.fire_outside_lua(&mut wa, false, reason).unwrap();
            wa.clear_sync_access(w);
        }
        {
            FAST_FLAGS.store::<FFSignalBehavior>(prev);
            FAST_FLAGS.store::<FFTaskSchedulerTimeSensitive>(true);
            let timer = Instant::now();
            let mut fake_world = Some(World::new());
            let mut waiting = containers_qs
                .iter(w)
                .map(|x| {
                    (
                        x.lua.clone(),
                        TaskScheduler::fetch(&x.lua).still_waiting_shutdown(),
                    )
                })
                .filter(|(_, waiting)| *waiting)
                .collect::<Vec<_>>();
            loop {
                for (lua, still_waiting) in waiting.iter_mut() {
                    if *still_waiting {
                        unsafe {
                            WorldAccess::fetch(lua).insert_sync_access(w);
                        }
                        let task = TaskScheduler::fetch(lua);
                        task.run(
                            lua,
                            false,
                            true,
                            Duration::from_secs_f64(0.03),
                            Some(
                                Duration::from_secs_f64(FAST_FLAGS.fetch::<FFShutdownTimeout>())
                                    .checked_sub(timer.elapsed())
                                    .unwrap_or_default(),
                            ),
                        );
                        WorldAccess::fetch(lua).clear_sync_access(w);
                        *still_waiting = task.still_waiting_shutdown();
                    }
                }
                let arc_world = Arc::new(take_world_local(&mut fake_world, w));
                let mut queue = CommandQueue::default();
                for (lua, still_waiting) in waiting.iter_mut() {
                    if *still_waiting {
                        unsafe {
                            WorldAccess::fetch(lua).insert_desync_access(arc_world.clone());
                        }
                        let task = TaskScheduler::fetch(lua);
                        task.run(
                            lua,
                            true,
                            true,
                            Duration::from_secs_f64(0.03),
                            Some(
                                Duration::from_secs_f64(FAST_FLAGS.fetch::<FFShutdownTimeout>())
                                    .checked_sub(timer.elapsed())
                                    .unwrap_or_default(),
                            ),
                        );
                        if let Some(mut q) = WorldAccess::fetch(lua).clear_desync_access() {
                            queue.append(&mut q);
                        }
                        *still_waiting = task.still_waiting_shutdown();
                    }
                }
                put_world_local(&mut fake_world, Arc::into_inner(arc_world).unwrap(), w);
                queue.apply(w);
                if !waiting.iter().any(|(_, f)| *f) {
                    break;
                }
            }
        }
        ext_post_shutdown(w);
        cleanup_instances(w);
        #[cfg(test)]
        {
            use crate::core::RblxLogs;

            Engine::assert_no_errors(w.resource::<RblxLogs>());
        }
    }
}

fast_flag!(FFShutdownTimeout: f64 = 30.0);
