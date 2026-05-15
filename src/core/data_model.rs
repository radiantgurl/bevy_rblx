use crate::{
    core::{FAST_FLAGS, Instance, WorldAccess, instance::RootInstance},
    enums::CreatorType,
    internal_prelude::*,
    userdata::{FFSignalBehavior, ObjectRef, RBXScriptSignal},
};
use bevy::prelude::*;
use mlua::prelude::*;

use super::ServiceProviderMembers;
use bevy_rblx_derive::{fast_flag, register_class};

register_class! {
    #[require_components(RootInstance)]
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
            Instance::find_first_child_of_class(lua, (ObjectRef::new(lua, this), "RunService".to_owned()))?.into_lua(lua)
        }]
        virtual run_service: ObjectRef,
        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            Instance::find_first_child_of_class(lua, (ObjectRef::new(lua, this), "Workspace".to_owned()))?.into_lua(lua)
        }]
        #[deprecated_alias="workspace"]
        virtual workspace: ObjectRef,

        priv closing_signal: RBXScriptSignal
    }
    methods {
        fn bind_to_close(lua: &Lua, this: ObjectRef, f: LuaFunction) -> LuaResult<()> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();

            let signal = &world.get::<DataModelMembers>(this.entity()).expect("this is a data model").closing_signal;
            signal.connect(lua, f)?;
            Ok(())
        }
    }
}

pub fn bind_close_system_runner(mut app_exit: MessageReader<AppExit>, mut c: Commands) {
    for _ in app_exit.read() {
        c.queue(|w: &mut World| {
            let closing_signal = w
                .query::<&DataModelMembers>()
                .single(w)
                .expect("root instance exists while exiting app")
                .closing_signal
                .reference();
            let close = w
                .query::<&ServiceProviderMembers>()
                .single(w)
                .expect("root instance exists while exiting app")
                .close
                .reference();
            {
                let mut wa = WorldAccess::default();
                let unsafe_world = w.as_unsafe_world_cell();
                unsafe {
                    wa.insert_sync_access(unsafe_world);
                }
                FAST_FLAGS.store::<FFSignalBehavior>(1);

                closing_signal.fire_outside_lua(&mut wa, false, ()).unwrap();
                close.fire_outside_lua(&mut wa, false, ()).unwrap();
                wa.clear_desync_access();
            }
        })
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
