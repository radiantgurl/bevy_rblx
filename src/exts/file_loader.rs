use std::{fs::File, io::BufReader};

use crate::{
    core::{
        FAST_FLAGS, LuauContainer, SchedulerPhase, WorldAccess,
        extension::{EngineExtensionDistribution, EngineExtensionInitLevel},
        object::{FFGameName, data_model::DataModelMembers},
    },
    internal::{EngineExtension, OBJECT_VTABLES},
    internal_prelude::*,
};
use bevy::{ecs::schedule::ScheduleCleanupPolicy, prelude::*, tasks::AsyncComputeTaskPool};
use bevy_async_commands::prelude::*;
use bevy_async_ecs::AsyncWorld;
use bevy_rblx_derive::register;
use rbx_binary::from_reader;

#[derive(Message, Clone, Debug)]
pub struct LoadPlace(pub String);

#[derive(Message, Clone, Debug)]
pub struct LoadModel {
    pub path: String,
    pub parent: Entity,
}

#[derive(Message)]
pub enum LoadResult {
    Successful(String),
    Failed(String, BevyError),
}

impl LoadResult {
    pub fn path(&self) -> &str {
        match self {
            LoadResult::Successful(path) | LoadResult::Failed(path, _) => path.as_str(),
        }
    }
    pub fn is_successful(&self) -> bool {
        match self {
            LoadResult::Successful(_) => true,
            _ => false,
        }
    }
    pub fn as_result(&self) -> Result<(), &BevyError> {
        match self {
            LoadResult::Successful(_) => Ok(()),
            LoadResult::Failed(_, e) => Err(e),
        }
    }
}
#[derive(Clone, Copy, Default)]
struct RblxFileLoader;

#[register]
impl EngineExtension for RblxFileLoader {
    fn id(&self) -> &'static str {
        "fileloader"
    }

    fn init_level(&self) -> EngineExtensionInitLevel {
        EngineExtensionInitLevel::Runtime
    }

    fn distribution(&self) -> EngineExtensionDistribution {
        EngineExtensionDistribution::Both
    }

    fn dyn_clone(&mut self, _app: &mut App) -> Box<dyn EngineExtension> {
        Box::new(self.clone())
    }

    fn description(&self) -> Option<&'static str> {
        Some("Loads .rbxm and .rbxl files")
    }

    fn dynamically_removable(&self) -> bool {
        true
    }

    fn default_enabled(&self) -> bool {
        true
    }

    fn runtime_init(&self, world: &mut World) {
        world.schedule_scope(First, |_, s| {
            s.add_systems(
                file_loader_system
                    .before(SchedulerPhase::ReplicationRecv)
                    .in_set(FileLoaderSet),
            );
        })
    }

    fn post_shutdown_hook(&self, world: &mut World) {
        world.schedule_scope(First, |w, s| {
            s.remove_systems_in_set(FileLoaderSet, w, ScheduleCleanupPolicy::RemoveSetAndSystems)
                .unwrap();
        })
    }
}

#[derive(SystemSet, Hash, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileLoaderSet;

pub async fn load_place(
    async_world: AsyncWorld,
    game: Entity,
    path: &String,
) -> Result<(), BevyError> {
    let file = File::open(path)?;
    let buf_file = BufReader::new(file);
    let opened_place = from_reader(buf_file)?;

    FAST_FLAGS.store::<FFGameName>(opened_place.root().name.clone());
    async_world
        .entity(game)
        .insert(Name::new(opened_place.root().name.clone()))
        .await;

    // validate services
    let services = opened_place
        .root()
        .children()
        .iter()
        .filter_map(|r| opened_place.get_by_ref(*r))
        .filter_map(|i| {
            let class_name = i.class.to_string();
            if OBJECT_VTABLES
                .get(class_name.as_str())
                .map(|vtable| vtable.inherits.contains(&"Service"))
                .unwrap_or_default()
            {
                Some(class_name)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    async_world
        .apply(move |w: &mut World| {
            let mut main_state_query = w.query_filtered::<&LuauContainer, With<DataModelMembers>>();
            let main_state = main_state_query.single(w).unwrap().lua.clone();
            // WorldAccess::fetch(&main_state).
        })
        .await;
    Ok(())
}

pub async fn load_model(
    async_world: AsyncWorld,
    path: &String,
    parent: Entity,
) -> Result<(), BevyError> {
    let file = File::open(path)?;
    let buf_file = BufReader::new(file);
    let opened_place = from_reader(buf_file)?;

    Ok(())
}

async fn handle_result(res: Result<(), BevyError>, path: String, async_world: AsyncWorld) {
    match res {
        Ok(_) => async_world.send_message(LoadResult::Successful(path)).await,
        Err(e) => {
            bevy::log::error!(target: "bevy_rblx::file_loader", "An error occurred while loading file {path:?}: {e}");
            async_world.send_message(LoadResult::Failed(path, e)).await;
        }
    }
}

pub fn file_loader_system(
    mut model_requests: MessageReader<LoadModel>,
    mut place_requests: MessageReader<LoadPlace>,
    mut commands: Commands,
    game: Single<Entity, With<DataModelMembers>>,
) {
    for LoadPlace(path) in place_requests.read().cloned() {
        let async_world = commands.async_world();
        let game = *game;
        AsyncComputeTaskPool::get()
            .spawn(async move {
                let async_world = async_world.await;
                let res = load_place(async_world.clone(), game, &path).await;
                handle_result(res, path, async_world).await;
            })
            .detach();
    }
    for LoadModel { path, parent } in model_requests.read().cloned() {
        let async_world = commands.async_world();
        AsyncComputeTaskPool::get()
            .spawn(async move {
                let async_world = async_world.await;
                let res = load_model(async_world.clone(), &path, parent).await;
                handle_result(res, path, async_world).await;
            })
            .detach();
    }
}
