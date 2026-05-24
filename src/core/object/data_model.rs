use crate::{
    core::{FAST_FLAGS, ShutdownReason, lua::{LuauContainer, WorldAccess}, object::{DisabledObject, InstanceMembers, service_provider::{ServiceProvider, ServiceProviderMembers}}}, enums::{CloseReason, CreatorType}, internal_prelude::*, userdata::ObjectRef
};
use bevy::prelude::*;
use mlua::prelude::*;
use bevy_rblx_derive::{fast_flag, register_class};

#[derive(Clone, Copy, Component, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct RootInstance;

register_class! {
    #[require_components(RootInstance, LuauContainer)]
    #[post_init=fn (lua: &Lua, this: Entity) -> LuaResult<()> {
        let mut wa = WorldAccess::fetch(lua);
        let world = wa.access_synchronized()?;
        world.get_mut::<Name>(this).unwrap().set(FAST_FLAGS.fetch::<FFGameName>());
        let mut m = world.get_mut::<InstanceMembers>(this).unwrap();
        m.cloning_protected = true;
        m.destroy_protected = true;
        m.parent_protected = true;
        Ok(())
    }]
    priv DataModel(ServiceProvider)
    members {
        #[getter=fn(lua: &Lua, _this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            FAST_FLAGS.fetch::<FFGameCreatorId>().into_lua(lua)
        }]
        virtual creator_id: u64,
        #[getter=fn(lua: &Lua, _this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            match FAST_FLAGS.fetch::<FFGameCreatorType>() {
                0 => CreatorType::User,
                1 => CreatorType::Group,
                _ => unreachable!("invalid creator type fastflag enum")
            }.into_lua(lua)
        }]
        virtual creator_type: CreatorType,
        #[getter=fn(lua: &Lua, _this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            FAST_FLAGS.fetch::<FFGameId>().into_lua(lua)
        }]
        virtual game_id: u64,
        #[getter=fn(lua: &Lua, _this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            FAST_FLAGS.fetch::<FFJobId>().into_lua(lua)
        }]
        virtual job_id: u64,
        #[getter=fn(lua: &Lua, _this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            FAST_FLAGS.fetch::<FFPlaceId>().into_lua(lua)
        }]
        virtual place_id: u64,
        #[getter=fn(lua: &Lua, _this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            FAST_FLAGS.fetch::<FFPlaceVersion>().into_lua(lua)
        }]
        virtual place_version: u64,
        #[getter=fn(lua: &Lua, _this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            FAST_FLAGS.fetch::<FFPrivateServerId>().into_lua(lua)
        }]
        #[deprecated_alias="VIPServerId"]
        virtual private_server_id: String,
        #[getter=fn(lua: &Lua, _this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            FAST_FLAGS.fetch::<FFPrivateServerOwnerId>().into_lua(lua)
        }]
        #[deprecated_alias="VIPServerOwnerId"]
        virtual private_server_owner_id: u64,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            ServiceProvider::find_service(lua, (ObjectRef::new(lua, this), "RunService".to_owned()))?.into_lua(lua)
        }]
        virtual run_service: ObjectRef,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            ServiceProvider::get_service(lua, (ObjectRef::new(lua, this), "Workspace".to_owned()))?.into_lua(lua)
        }]
        #[deprecated_alias="workspace"]
        virtual workspace: ObjectRef
    }
    methods {
        fn bind_to_close(lua: &Lua, this: ObjectRef, f: LuaFunction) -> LuaResult<()> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();

            let signal = &world.get::<ServiceProviderMembers>(this.entity()).expect("this is a data model").close;
            signal.connect(lua, f)?;
            Ok(())
        }
        fn shutdown(lua: &Lua, this: ObjectRef, reason: Option<CloseReason>) -> LuaResult<()> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let mut commands = world_access.access_commands();
            if let Some(r) = reason {
                commands.insert_resource(ShutdownReason(r));
            }
            commands.write_message(AppExit::Success);
            Ok(())
        }
    }
}

pub fn register_game_global(w: &mut World) {
    let game = w
        .query_filtered::<Entity, With<RootInstance>>()
        .single(w)
        .unwrap();
    let containers = w
        .query_filtered::<&LuauContainer, (Added<LuauContainer>, Allow<DisabledObject>)>()
        .iter(w)
        .map(|x| x.lua.clone())
        .collect::<Vec<_>>();
    for lua in containers {
        unsafe {
            WorldAccess::fetch(&lua).insert_sync_access(w);
            lua.globals()
                .raw_set("game", ObjectRef::new(&lua, game))
                .unwrap();
        }
        WorldAccess::fetch(&lua).clear_sync_access(w)
    }
}

fast_flag!(FFGameCreatorId: u64 = 0);
fast_flag!(FFGameCreatorType: u64 = 0);
fast_flag!(FFGameId: u64 = 0);
fast_flag!(FFJobId: String = "".to_owned());
fast_flag!(FFPlaceId: u64 = 0);
fast_flag!(FFPlaceVersion: u64 = 1);
fast_flag!(FFPrivateServerId: String = "reserved server".to_owned());
fast_flag!(FFPrivateServerOwnerId: u64 = 0);
fast_flag!(FFGameName: String = "bevy_rblx test instance".to_owned());
