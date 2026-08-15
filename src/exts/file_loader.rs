use std::{fs::File, io::BufReader};

use crate::{
    core::{
        FAST_FLAGS, LuauContainer, SchedulerPhase, WorldAccess,
        extension::{EngineExtensionDistribution, EngineExtensionInitLevel},
        object::{
            FFGameName, InstanceMembers, data_model::DataModelMembers,
            service_provider::ServiceProvider,
        },
    },
    internal::{EngineExtension, OBJECT_VTABLES},
    internal_prelude::*,
    userdata::{LuaFreeValue, ObjectRef},
};
use bevy::{
    ecs::schedule::ScheduleCleanupPolicy, platform::collections::HashMap, prelude::*,
    tasks::AsyncComputeTaskPool,
};
use bevy_async_commands::prelude::*;
use bevy_async_ecs::AsyncWorld;
use bevy_rblx_derive::{register, register_class};
use rbx_binary::from_reader;

use mlua::prelude::*;
use rbx_types::Variant;

/// Emitted whenever its time to load a place file into the engine. Typically emitted from within
/// the engine.
///
/// Internally calls [load_place] with an async system context.
#[derive(Message, Clone, Debug)]
pub struct LoadPlace(
    /// The path for the .rbxl file
    pub String,
);

/// Emitted whenever its time to load a model file into the engine. Typically emitted from within
/// the engine.
///
/// Internally calls [load_model] with an async system context.
#[derive(Message, Clone, Debug)]
pub struct LoadModel {
    /// The path for the .rbxm file
    pub path: String,
    /// The parent of the model instance
    pub parent: Entity,
}
/// The result of a load operation after emitting LoadModel or LoadPlace.
/// You can find which file it was for by checking the .path() method.
#[derive(Message)]
pub enum LoadResult {
    /// The load was successful for path [.0]
    Successful(String),
    /// The load failed for path [.0] with the error [.1]
    Failed(String, BevyError),
}

impl LoadResult {
    /// The path of the load request that this result originates from
    pub fn path(&self) -> &str {
        match self {
            LoadResult::Successful(path) | LoadResult::Failed(path, _) => path.as_str(),
        }
    }
    /// If the load was successful
    pub fn is_successful(&self) -> bool {
        match self {
            LoadResult::Successful(_) => true,
            _ => false,
        }
    }
    /// Convert into a normal result
    pub fn as_result(&self) -> Result<(), &BevyError> {
        match self {
            LoadResult::Successful(_) => Ok(()),
            LoadResult::Failed(_, e) => Err(e),
        }
    }
}
/// The [EngineExtension] in charge of loading files, known internally as `file_loader`.
///
/// Can be loaded and unloaded at runtime.
#[derive(Clone, Copy, Default)]
struct RblxFileLoader;

#[register]
impl EngineExtension for RblxFileLoader {
    fn id(&self) -> &'static str {
        "file_loader"
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
        world.insert_resource(Messages::<LoadPlace>::default());
        world.insert_resource(Messages::<LoadModel>::default());
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

fn variant_to_data(l: &Lua, v: Variant) -> LuaResult<LuaValue> {
    match v {
        Variant::Bool(v) => v.into_lua(l),
        Variant::String(v) => v.into_lua(l),
        Variant::Float32(v) => v.into_lua(l),
        Variant::Float64(v) => v.into_lua(l),
        Variant::Int32(v) => v.into_lua(l),
        Variant::Int64(v) => v.into_lua(l),
        Variant::BinaryString(binary_string) => {
            LuaFreeValue::Buffer(binary_string.into_vec()).into_lua(l)
        }
        Variant::Axes(axes) => todo!(),
        Variant::BrickColor(brick_color) => todo!(),
        Variant::CFrame(cframe) => todo!(),
        Variant::Color3(color3) => todo!(),
        Variant::Color3uint8(color3uint8) => todo!(),
        Variant::ColorSequence(color_sequence) => todo!(),
        Variant::ContentId(content_id) => todo!(),
        Variant::Enum(_) => todo!(),
        Variant::Faces(faces) => todo!(),
        Variant::NumberRange(number_range) => todo!(),
        Variant::NumberSequence(number_sequence) => todo!(),
        Variant::PhysicalProperties(physical_properties) => todo!(),
        Variant::Ray(ray) => todo!(),
        Variant::Rect(rect) => todo!(),
        Variant::Ref(_) => todo!(),
        Variant::Region3(region3) => todo!(),
        Variant::Region3int16(region3int16) => todo!(),
        Variant::SharedString(shared_string) => todo!(),
        Variant::UDim(udim) => todo!(),
        Variant::UDim2(udim2) => todo!(),
        Variant::Vector2(vector2) => todo!(),
        Variant::Vector2int16(vector2int16) => todo!(),
        Variant::Vector3(vector3) => todo!(),
        Variant::Vector3int16(vector3int16) => todo!(),
        Variant::OptionalCFrame(cframe) => todo!(),
        Variant::Tags(tags) => todo!(),
        Variant::Attributes(attributes) => todo!(),
        Variant::Font(font) => todo!(),
        Variant::UniqueId(unique_id) => todo!(),
        Variant::MaterialColors(material_colors) => todo!(),
        Variant::SecurityCapabilities(security_capabilities) => todo!(),
        Variant::EnumItem(enum_item) => todo!(),
        Variant::Content(content) => todo!(),
        Variant::NetAssetRef(net_asset_ref) => todo!(),
        _ => todo!(),
    }
}

async fn load_recursive_from_ref(async_world: AsyncWorld) {
    todo!()
}

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
        .filter_map(|r| opened_place.get_by_ref(*r).map(|v| (*r, v)))
        .filter_map(|(r, i)| {
            let class_name = i.class.to_string();
            if OBJECT_VTABLES
                .get(class_name.as_str())
                .map(|vtable| vtable.inherits.contains(&"Service"))
                .unwrap_or(false)
            {
                Some((r, class_name))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let root_ref = opened_place.root_ref();
    let mut ref_entity_map = async_world
        .apply2(move |w: &mut World| -> LuaResult<_> {
            let mut ref_entity_map = HashMap::new();
            let mut main_state_query =
                w.query_filtered::<(&LuauContainer, Entity), With<DataModelMembers>>();
            let (main_state, game_entity) = {
                let (c, e) = main_state_query.single(w).unwrap();
                (c.lua.clone(), e)
            };
            ref_entity_map.insert(root_ref, game_entity);
            let _guard = WorldAccess::fetch(&main_state).insert_sync_access(w, &main_state);
            let game_ref = ObjectRef::new(&main_state, game_entity);
            for (r, service) in services {
                let oref = ServiceProvider::get_service(&main_state, (game_ref.clone(), service))?;
                if let Some(oref) = oref {
                    ref_entity_map.insert(r, oref.entity());
                }
            }
            Ok(ref_entity_map)
        })
        .await?;
    // all services have been properly loaded by this point
    // insert world access now
    for (i, r) in opened_place
        .root()
        .children()
        .iter()
        .filter_map(|r| opened_place.get_by_ref(*r).map(|i| (i, *r)))
    {
        if !ref_entity_map.contains_key(&r) {
            return Err(LuaError::runtime(format!(
                "failed to load top-level service of class: {r} (does the class not exist?)"
            ))
            .into());
        }
        // ref valid
    }
    Ok(())
}

pub async fn load_model(
    async_world: AsyncWorld,
    path: &String,
    parent: Entity,
) -> Result<(), BevyError> {
    let file = File::open(path)?;
    let buf_file = BufReader::new(file);
    let opened_model = from_reader(buf_file)?;

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

/// Handles all incoming calls for the [LoadModel] and [LoadPlace] events and properly calls the
/// corresponding [load_model] and [load_place] systems.
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

register_class! {
    priv MissingInstance(Instance)
    members {
        pub priv members: HashMap<String, LuaFreeValue>
    }
    methods {}
}
