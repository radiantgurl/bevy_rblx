use bevy::ecs::query::FilteredAccessSet;
use bevy_rblx_derive::register_class;

use crate::core::WorldAccess;
use crate::core::templates::EraseOnClone;
use crate::enums::BulkMoveMode;
use crate::internal_prelude::*;
use crate::userdata::{CFrame, ObjectRef};
use avian3d::prelude::*;
use bevy::ecs::system::{SystemMeta, SystemParam, SystemState};
use bevy::prelude::*;
use mlua::prelude::*;

use crate::core::object::ServiceMembers;
use crate::instance::ModelMembers;

type QueryState = <SpatialQuery<'static, 'static> as SystemParam>::State;

fn fetch_query<'w, 's>(
    w: &'w World,
    state: &'s mut QueryState,
    meta: &'w SystemMeta,
) -> LuaResult<SpatialQuery<'w, 's>> {
    let tick = w.read_change_tick();
    let unsafe_world = w.as_unsafe_world_cell_readonly();
    unsafe {
        <SpatialQuery<'w, 's> as SystemParam>::validate_param(state, meta, unsafe_world)
            .into_lua_err()?;
        Ok(<SpatialQuery<'w, 's> as SystemParam>::get_param(
            state,
            meta,
            unsafe_world,
            tick,
        ))
    }
}

register_class! {
    #[post_init=fn(lua: &Lua, this: Entity) -> LuaResult<()> {
        let mut wa = WorldAccess::fetch(lua);
        let world = wa.access_synchronized()?;

        let state = <SpatialQuery<'static, 'static> as SystemParam>::init_state(world);
        let mut meta = SystemState::<SpatialQuery<'static, 'static>>::new(world).meta().clone();

        let mut access_set = FilteredAccessSet::new();
        let mut read_only_access_set = FilteredAccessSet::new();
        read_only_access_set.read_all();

        <SpatialQuery<'static, 'static> as SystemParam>::init_access(&state, &mut meta, &mut access_set, world);

        if !access_set.is_compatible(&read_only_access_set) {
            return Err(LuaError::runtime("failed read-only check during building of query state"));
        }

        let mut members = WorldRootMembers::fetch_members_mut(world, this);

        *members.query_state = Some(state);
        *members.meta = Some(meta);

        Ok(())
    }]
    #[reflect_opaque]
    abstract WorldRoot(Model)
    members {
        priv query_state: EraseOnClone<Option<QueryState>>,
        priv meta: EraseOnClone<Option<SystemMeta>>
    }
    methods {
        fn bulk_move_to(lua: &Lua, _this: ObjectRef, instances: Vec<ObjectRef>, cframes: Vec<CFrame>, event_mode: Option<BulkMoveMode>) -> LuaResult<()> {
            if instances.len() != cframes.len() {
                return Err(LuaError::runtime(format!("expected number of instances({}) to match number of cframes ({})", instances.len(), cframes.len())))
            }
            let event_mode = event_mode.unwrap_or(BulkMoveMode::FireAllEvents);
            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;
            let mut objects_qs = world.query::<&crate::core::object::ObjectHeader>();
            if !objects_qs.iter_many(world, instances.iter().map(|x| x.entity()).collect::<Vec<_>>()).all(|o| o.vtable.is_a("BasePart")) {
                return Err(LuaError::runtime("expected arg 2 an array of baseparts"));
            }
            Ok(())
        }
    }
}

register_class! {
    priv Workspace(WorldRoot, Service)
    members {}
    methods {}
}
