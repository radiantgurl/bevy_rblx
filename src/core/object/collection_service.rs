use bevy::{
    ecs::entity::Entity,
    platform::collections::{HashMap, HashSet},
};

use crate::{core::bevy::RefCounted, internal_prelude::*};
use bevy_rblx_derive::register_class;
use mlua::prelude::*;

use crate::{
    core::{WorldAccess, object::ServiceMembers},
    userdata::{LuaSendRBXScriptConnection, ObjectRef, RBXScriptSignal},
};

use super::InstanceMembers;

fn on_destroy(lua: &Lua, (this, instance): (ObjectRef, ObjectRef)) -> LuaResult<()> {
    let tags = {
        let world_access = WorldAccess::fetch_readonly(lua);
        let world = world_access.access_read_only();
        let members = world
            .get::<CollectionServiceMembers>(this.entity())
            .expect("this is CollectionService");
        members
            .rev_instances
            .get(&instance.entity())
            .expect("instance has tags still")
            .iter()
            .cloned()
            .collect::<Vec<_>>()
    };
    let remove_tag = lua.create_function(CollectionService::remove_tag)?;
    for tag in tags.into_iter() {
        remove_tag.queue_call(lua, (this.clone_lua(lua), instance.clone_lua(lua), tag))?;
    }
    Ok(())
}

register_class! {
    priv CollectionService (Service)
    members {
        priv instances: HashMap<String, HashSet<Entity>>,
        priv rev_instances: HashMap<Entity, HashSet<String>>,
        priv destroying_conns: HashMap<Entity, LuaSendRBXScriptConnection>,
        priv tag_signals: HashMap<String, (RBXScriptSignal, RBXScriptSignal)>,

        #[read_only]
        pub tag_added: RBXScriptSignal,
        #[read_only]
        pub tag_removed: RBXScriptSignal
    }
    methods {
        fn add_tag(lua: &Lua, this: ObjectRef, instance: ObjectRef, tag: String) -> LuaResult<()> {
            let (tag_added, instance_added) = {
                let mut world_access = WorldAccess::fetch(lua);
                let world = world_access.access_synchronized()?;
                let (tag_added, instance_added) = {
                    let mut mut_ref = world.get_mut::<CollectionServiceMembers>(this.entity()).expect("is collection service");
                    match &mut *mut_ref {
                        CollectionServiceMembers { instances, tag_signals, tag_added, rev_instances, .. } => {
                            let tags = rev_instances.entry(instance.entity()).or_default();
                            if tags.contains(&tag) {
                                return Ok(()); // already has tag
                            }
                            tags.insert(tag.clone());
                            if let Some(instances) = instances.get_mut(&tag)  {
                                instances.insert(instance.entity());
                                (None, tag_signals.get(&tag).map(|x| x.0.reference()))
                            } else {
                                let mut set = HashSet::new();
                                set.insert(instance.entity());
                                instances.insert(tag.clone(), set);
                                (Some(tag_added.reference()), tag_signals.get(&tag).map(|x| x.0.reference()))
                            }
                        }
                    }
                };
                let instance_destroying = world.get_mut::<InstanceMembers>(instance.entity()).expect("is instance").destroying.reference();
                let new_instance = unsafe {
                    let instance_cloned = instance.clone_no_inc_ref();
                    world.get_mut::<RefCounted>(instance_cloned.entity()).expect("instances are ref counted").inc();
                    instance_cloned
                };
                let mut mut_ref = world.get_mut::<CollectionServiceMembers>(this.entity()).expect("is collection service");
                mut_ref.destroying_conns.entry(instance.entity()).or_insert_with(|| {
                    let func = lua.create_function(on_destroy).unwrap().bind((this.into_lua(lua).unwrap(), new_instance.into_lua(lua).unwrap())).unwrap();
                    instance_destroying.connect(lua, func).unwrap().into_sendable(lua).expect("no error while making connection sendable")
                });
                (tag_added, instance_added)
            };
            if let Some(event) = tag_added {
                event.fire_in_lua(lua, false, tag.clone())?;
            }
            if let Some(event) = instance_added {
                event.fire_in_lua(lua, false, instance)?;
            }
            Ok(())
        }
        fn get_all_tags(lua: &Lua, this: ObjectRef) -> LuaResult<LuaValue> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();

            let members = world.get::<CollectionServiceMembers>(this.entity()).expect("is collection service members");
            members.instances.keys().cloned().collect::<Vec<_>>().into_lua(lua)
        }
        fn get_instance_added_signal(lua: &Lua, this: ObjectRef, tag: String) -> LuaResult<LuaValue> {
            let mut world_access = WorldAccess::fetch(lua);
            let world = world_access.access_synchronized()?;

            let mut members = world.get_mut::<CollectionServiceMembers>(this.entity()).expect("is collection service members");
            members.tag_signals.entry(tag).or_default().0.into_lua(lua)
        }
        fn get_instance_removed_signal(lua: &Lua, this: ObjectRef, tag: String) -> LuaResult<LuaValue> {
            let mut world_access = WorldAccess::fetch(lua);
            let world = world_access.access_synchronized()?;

            let mut members = world.get_mut::<CollectionServiceMembers>(this.entity()).expect("is collection service members");
            members.tag_signals.entry(tag).or_default().1.into_lua(lua)
        }
        fn get_tagged(lua: &Lua, this: ObjectRef, tag: String) -> LuaResult<LuaValue> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();

            let members = world.get::<CollectionServiceMembers>(this.entity()).expect("is collection service members");
            members.instances.get(&tag).cloned().unwrap_or_default().into_iter().map(|e| ObjectRef::new(lua, e)).collect::<Vec<_>>().into_lua(lua)
        }
        fn get_tags(lua: &Lua, this: ObjectRef, instance: ObjectRef) -> LuaResult<LuaValue> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();

            let members = world.get::<CollectionServiceMembers>(this.entity()).expect("is collection service members");
            members.rev_instances.get(&instance.entity()).cloned().unwrap_or_default().into_iter().collect::<Vec<_>>().into_lua(lua)
        }
        fn has_tag(lua: &Lua, this: ObjectRef, instance: ObjectRef, tag: String) -> LuaResult<bool> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();

            let members = world.get::<CollectionServiceMembers>(this.entity()).expect("is collection service members");
            Ok(members.rev_instances.get(&instance.entity()).cloned().unwrap_or_default().contains(&tag))
        }
        fn remove_tag(lua: &Lua, this: ObjectRef, instance: ObjectRef, tag: String) -> LuaResult<()> {
            let (tag_removed_event, instance_removed_event) = {
                let mut world_access = WorldAccess::fetch(lua);
                let world = world_access.access_synchronized()?;
                let mut members = world.get_mut::<CollectionServiceMembers>(this.entity()).expect("is collection service");

                let had_tag = if let Some(instances) = members.instances.get_mut(&tag) {
                    instances.remove(&instance.entity())
                } else {
                    return Ok(()); // tag doesn't exist
                };
                if !had_tag {
                    return Ok(());
                }

                members.rev_instances.get_mut(&instance.entity()).unwrap().remove(&tag);

                let ev = if members.instances.get(&tag).unwrap().is_empty() {
                    members.instances.remove(&tag);
                    Some(members.tag_removed.reference())
                } else {
                    None
                };
                (ev, members.tag_signals.get(&tag).map(|x| x.1.reference()))
            };
            if let Some(ev) = instance_removed_event {
                ev.fire_in_lua(lua, false, instance)?;
            }
            if let Some(ev) = tag_removed_event {
                ev.fire_in_lua(lua, false, tag)?;
            }
            Ok(())
        }
    }
}
