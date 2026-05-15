use std::mem::take;

use crate::core::luau::ContainerProvenance;
use crate::enums::MessageType;
use crate::userdata::{LuaFreeValue, RBXScriptSignal};
use crate::{core::WorldAccess, internal_prelude::*, userdata::ObjectRef};
use bevy::ecs::entity::EntityCloner;
use bevy::prelude::*;

use bevy::{
    ecs::{component::Component, hierarchy::Children, name::Name, system::EntityCommands},
    platform::collections::HashMap,
};
use bevy_rblx_derive::register_class;
use lazy_static::lazy_static;
use mlua::prelude::*;

use crate::core::object::ObjectNewFn;
use crate::core::{CollectionService, DataModel, ObjectHeader, ObjectVTableCreationPointer, ServiceProvider, push_log};

pub struct InstanceConstructor {
    visible: HashMap<&'static str, fn(&Lua, EntityCommands) -> LuaResult<()>>,
    all: HashMap<&'static str, fn(&Lua, EntityCommands) -> LuaResult<()>>,
}

lazy_static! {
    pub static ref INSTANCE_CONSTRUCTOR: InstanceConstructor = {
        let all = inventory::iter::<ObjectVTableCreationPointer>
            .into_iter()
            .map(|x| x.0())
            .map(|x| (x.class_name, &x.new))
            .filter_map(|(k, v)| {
                Some((
                    k,
                    match v {
                        ObjectNewFn::None => return None,
                        ObjectNewFn::Protected(f) | ObjectNewFn::Visible(f) => *f,
                    },
                ))
            })
            .collect::<HashMap<_, _>>();
        let visible = inventory::iter::<ObjectVTableCreationPointer>
            .into_iter()
            .map(|x| x.0())
            .map(|x| (x.class_name, &x.new))
            .filter_map(|(k, v)| {
                Some((
                    k,
                    match v {
                        ObjectNewFn::None | ObjectNewFn::Protected(_) => return None,
                        ObjectNewFn::Visible(f) => *f,
                    },
                ))
            })
            .collect::<HashMap<_, _>>();

        InstanceConstructor { visible, all }
    };
}

#[derive(Message, Clone, Copy)]
pub struct NewInstanceEvent(pub Entity);

impl InstanceConstructor {
    pub fn new(&self, lua: &Lua, mut entity: EntityCommands, class_name: &str) -> LuaResult<()> {
        if let Some(x) = self.all.get(class_name) {
            x(lua, entity.reborrow())?;
            {
                let e = entity.id();
                entity.commands_mut().write_message(NewInstanceEvent(e));
                Ok(())
            }
        } else {
            Err(LuaError::runtime(format!(
                "Cannot construct instance of type {class_name}"
            )))
        }
    }
    pub fn protected_new(
        &self,
        lua: &Lua,
        mut entity: EntityCommands,
        class_name: &str,
    ) -> LuaResult<()> {
        if let Some(x) = self.visible.get(class_name) {
            x(lua, entity.reborrow())?;
            {
                let e = entity.id();
                entity.commands_mut().write_message(NewInstanceEvent(e));
                Ok(())
            }
        } else {
            Err(LuaError::runtime(format!(
                "Cannot construct instance of type {class_name}"
            )))
        }
    }
}

#[derive(Clone, Copy, Component, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct RootInstance;

pub fn remove_parent(lua: &Lua, this: Entity, new_parent: Option<Entity>) -> LuaResult<()> {
    let mut events: Vec<RBXScriptSignal> = Vec::new();
    {
        let mut wa = WorldAccess::fetch(lua);
        let world = wa.access_synchronized()?;

        let mut ancestors_qs = world.query::<&ChildOf>();
        let ancestors = ancestors_qs.query(&*world);
        let parent = ancestors.get(this.entity()).ok().map(|x| x.0);
        if parent.is_none() {
            return Ok(());
        }
        
        for a in ancestors.iter_ancestors(this.entity()) {
            let ancestor_members = InstanceMembers::fetch_members(&*world, a);
            events.push(ancestor_members.descendant_removing.clone());
        }
    }
    for ev in take( &mut events) {
        ev.fire_in_lua(lua, true, ObjectRef::new(lua, this))?
    }
    
    let child_removed = {
        let mut wa = WorldAccess::fetch(lua);
        let world = wa.access_synchronized()?;
        let parent = world.get::<ChildOf>(this).unwrap().0;
        world.entity_mut(parent).remove::<ChildOf>();
        
        let mut descendants_qs = world.query::<&Children>();
        let descendants = descendants_qs.query(world);

        for descendant in descendants.iter_descendants(this) {
            let descendant_members = InstanceMembers::fetch_members(&*world, descendant);
            events.push(descendant_members.ancestry_changed.clone());
        }
        
        let parent_members = InstanceMembers::fetch_members(&*world, parent);
        parent_members.child_removed.clone()
    };

    child_removed.fire_in_lua(lua, true, ObjectRef::new(lua, this))?;
    let new_parent = new_parent.map(|x| ObjectRef::new(lua, x));
    for ev in events {
        ev.fire_in_lua(lua, true, (ObjectRef::new(lua, this), new_parent.clone()))?;
    }

    Ok(())
}
pub fn add_parent(lua: &Lua, this: Entity, new_parent: Entity) -> LuaResult<()> {
    let mut events: Vec<RBXScriptSignal> = Vec::new();
    {
        let mut wa = WorldAccess::fetch(lua);
        let world = wa.access_synchronized()?;
        world.entity_mut(new_parent).add_child(this);
        let mut ancestors_qs = world.query::<&ChildOf>();
        let ancestors = ancestors_qs.query(&*world);
        events.push(InstanceMembers::fetch_members(world, this).child_added.clone());
        for ancestor in ancestors.iter_ancestors(this) {
            events.push(InstanceMembers::fetch_members(world, ancestor).descendant_added.clone())
        }
    }

    Ok(())
}

register_class! {
    #[require_components(Name, Children)]
    abstract Instance(Object)
    members {
        priv attributes: HashMap<String, LuaFreeValue>,
        priv attribute_changed_signal: HashMap<String, RBXScriptSignal>,

        #[default=false]
        priv destroyed: bool,
        #[default=false]
        pub priv cloning_protected: bool,
        #[default=false]
        pub priv destroy_protected: bool,
        #[default=false]
        pub priv parent_protected: bool,

        #[default=true]
        #[deprecated_alias="archivable"]
        pub archivable: bool,

        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();
            world.get::<Name>(this).expect("all instances have a name").as_str().into_lua(lua)
        }]
        #[setter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable, value: LuaValue) -> LuaResult<bool> {
            let mut world_access = WorldAccess::fetch(lua);
            let world = world_access.access_synchronized()?;
            let mut name = world.get_mut::<Name>(this).expect("all instances have a name");
            let new_name = value.to_string()?;
            let diff = name.as_str() != &new_name;
            if !diff {
                name.set(new_name);
            }
            Ok(diff)
        }]
        #[deprecated_alias="name"]
        virtual name: String,

        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();

            if let Some(parent) = world.get::<ChildOf>(this) {
                ObjectRef::new(lua, parent.0).into_lua(lua)
            } else {
                Ok(LuaValue::Nil)
            }
        }]
        #[setter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable, new_value: LuaValue) -> LuaResult<bool> {
            let obj_ref = Option::<ObjectRef>::from_lua(new_value, lua)?;
            {
                let wa = WorldAccess::fetch_readonly(lua);
                let world = wa.access_read_only();
                if world.get::<ChildOf>(this).map(|c| c.0) == obj_ref.as_ref().map(|x| x.entity()) {
                    return Ok(false); // no update
                }
                let members = InstanceMembers::fetch_members(&*world, this);
                if members.parent_protected || members.destroyed {
                    push_log(lua, MessageType::MessageWarning, format!("Failed to change instance Parent property, the property is locked."));
                    return Ok(false); // no update
                }
            }

            // This internal order is: DescendantRemoving -> ChildRemoved -> AncestryChanged -> ChildAdded -> DescendantAdded
            remove_parent(lua, this, obj_ref.as_ref().map(|x| x.entity()))?;
            if let Some(new_parent) = obj_ref {
                add_parent(lua, this, new_parent.entity())?;
            }
            Ok(true)
        }]
        #[deprecated_alias="parent"]
        virtual parent: Option<ObjectRef>,

        // sandboxed: bool,
        #[read_only]
        pub ancestry_changed: RBXScriptSignal,
        #[read_only]
        pub attribute_changed: RBXScriptSignal,
        #[deprecated_alias="childAdded"]
        #[read_only]
        pub child_added: RBXScriptSignal,
        #[read_only]
        pub child_removed: RBXScriptSignal,
        #[read_only]
        pub descendant_added: RBXScriptSignal,
        #[read_only]
        pub descendant_removing: RBXScriptSignal,
        #[read_only]
        pub destroying: RBXScriptSignal,
        #[read_only]
        pub styled_properties_changed: RBXScriptSignal
    }
    methods {
        fn add_tag(lua: &Lua, this: ObjectRef, tag: String) -> LuaResult<()> {
            let cs = {
                let mut wa = WorldAccess::fetch(lua);
                let world = wa.access_synchronized()?;
                
                let mut root_instance_qs = world.query_filtered::<Entity, With<RootInstance>>();
                let game_instance = root_instance_qs.single(world).expect("single game instance");

                let mut objects_qs = world.query::<&ObjectHeader>();
                let children = world.get::<Children>(game_instance).unwrap();
                children.iter().filter(|e| {
                    let header = objects_qs.get(world, *e).expect("expected object");

                    header.vtable.class_name == "CollectionService"
                }).next().expect("collection service exists")
            };
            CollectionService::add_tag(lua, (ObjectRef::new(lua, cs), this, tag))
        }
        fn clear_all_children(lua: &Lua, this: ObjectRef) -> LuaResult<()> {
            let children = {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();
                let members = world.get::<InstanceMembers>(this.entity()).expect("is instance");
                if members.destroy_protected {
                    return Ok(());
                }
                world.get::<Children>(this.entity()).expect("is instance").to_vec()
            };

            for i in children {
                Instance::destroy(lua, (ObjectRef::new(lua, i),))?;
            }
            Ok(())
        }
        #[deprecated_alias="clone"]
        fn clone(lua: &Lua, this: ObjectRef) -> LuaResult<Option<ObjectRef>> {
            {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();
                let members = world.get::<InstanceMembers>(this.entity()).expect("is instance");
                if members.cloning_protected {
                    push_log(lua, MessageType::MessageWarning, "object is not cloneable");
                    return Ok(None);
                }
                if !members.archivable {
                    return Ok(None)
                }
            }
            let new_instance = {
                let mut world_access = WorldAccess::fetch(lua);
                let world = world_access.access_synchronized()?;
                let new_instance = world.spawn_empty().id();
                let mut cloner = EntityCloner::default();
                cloner.clone_entity(world, this.entity(), new_instance);

                let all_instances = std::iter::once(new_instance).chain(world.query::<&Children>().query(world).iter_descendants(new_instance)).map(|x| NewInstanceEvent(x)).collect::<Vec<_>>();
                world.write_message_batch(all_instances);
                new_instance
            };
            // Ok(Some(ObjectRef::new(lua, new_instance)))
            todo!()
        }
        #[deprecated_alias="destroy"]
        fn destroy(lua: &Lua, this: ObjectRef) -> LuaResult<()> {
            let (vtable, destroying) = {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();
                let members = world.get::<InstanceMembers>(this.entity()).expect("is instance");
                if members.destroy_protected {
                    push_log(lua, MessageType::MessageWarning, "object is not destroyable");
                    return Ok(());
                }
                if members.destroyed {
                    push_log(lua, MessageType::MessageWarning, "object already destroyed");
                    return Ok(())
                }
                let vtable = world.get::<ObjectHeader>(this.entity()).expect("is object").vtable;
                let destroying = members.destroying.reference();
                (vtable, destroying)
            };

            destroying.fire_in_lua(lua, true, ())?;
            {
                let mut world_access = WorldAccess::fetch(lua);
                let world = world_access.access_synchronized()?;
                let mut members = world.get_mut::<InstanceMembers>(this.entity()).unwrap();

                members.parent_protected = false;
            }
            Instance::set_parent(lua, this.entity(), vtable, LuaValue::Nil)?;
            {
                let mut world_access = WorldAccess::fetch(lua);
                let world = world_access.access_synchronized()?;
                let mut members = world.get_mut::<InstanceMembers>(this.entity()).unwrap();
                members.destroyed = true;
                members.parent_protected = true;
            }
            Instance::clear_all_children(lua, (this, ))
        }
        fn find_first_ancestor(lua: &Lua, this: ObjectRef, name: String) -> LuaResult<Option<ObjectRef>> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();
            let mut query_state = world.try_query::<&ChildOf>().expect("query state was not initialized :(");
            let query = query_state.query(&*world);
            for ancestor in query.iter_ancestors(this.entity()) {
                if world.get::<Name>(ancestor).expect("expecting instance").as_str() == name {
                    return Ok(Some(ObjectRef::new(lua, ancestor)));
                }
            }
            Ok(None)
        }
        fn find_first_ancestor_of_class(lua: &Lua, this: ObjectRef, class_name: String) -> LuaResult<Option<ObjectRef>> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();
            let mut query_state = world.try_query::<&ChildOf>().expect("query state was not initialized :(");
            let query = query_state.query(&*world);
            for ancestor in query.iter_ancestors(this.entity()) {
                if world.get::<ObjectHeader>(ancestor).expect("expecting object").vtable.class_name == &class_name {
                    return Ok(Some(ObjectRef::new(lua, ancestor)));
                }
            }
            Ok(None)
        }
        fn find_first_ancestor_which_is_a(lua: &Lua, this: ObjectRef, class_name: String) -> LuaResult<Option<ObjectRef>> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();
            let mut query_state = world.try_query::<&ChildOf>().expect("query state was not initialized :(");
            let query = query_state.query(&*world);
            for ancestor in query.iter_ancestors(this.entity()) {
                for i in world.get::<ObjectHeader>(ancestor).expect("expecting object").vtable.method_resolution_order.iter() {
                    if i.class_name == &class_name {
                        return Ok(Some(ObjectRef::new(lua, ancestor)));
                    }
                }
            }
            Ok(None)
        }
        #[deprecated_alias="findFirstChild"]
        fn find_first_child(lua: &Lua, this: ObjectRef, name: String) -> LuaResult<Option<ObjectRef>> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();
            for child in world.get::<Children>(this.entity()).expect("instances have children").iter() {
                if world.get::<Name>(child).expect("expecting instance").as_str() == name {
                    return Ok(Some(ObjectRef::new(lua, child)));
                }
            }
            Ok(None)
        }
        fn find_first_child_of_class(lua: &Lua, this: ObjectRef, class_name: String) -> LuaResult<Option<ObjectRef>> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();
            for child in world.get::<Children>(this.entity()).expect("instances have children").iter() {
                if world.get::<ObjectHeader>(child).expect("expecting object").vtable.class_name == &class_name {
                    return Ok(Some(ObjectRef::new(lua, child)));
                }
            }
            Ok(None)
        }
        fn find_first_child_which_is_a(lua: &Lua, this: ObjectRef, class_name: String) -> LuaResult<Option<ObjectRef>> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();
            for child in world.get::<Children>(this.entity()).expect("instances have children").iter() {
                for i in world.get::<ObjectHeader>(child).expect("expecting object").vtable.method_resolution_order.iter() {
                    if i.class_name == &class_name {
                        return Ok(Some(ObjectRef::new(lua, child)));
                    }
                }
            }
            Ok(None)
        }
        fn find_first_descendant(lua: &Lua, this: ObjectRef, name: String) -> LuaResult<Option<ObjectRef>> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();
            let mut query_state = world.try_query::<&Children>().expect("query state was not initialized :(");
            let query = query_state.query(&*world);
            for descendant in query.iter_descendants(this.entity()) {
                if world.get::<Name>(descendant).expect("expecting instance").as_str() == name {
                    return Ok(Some(ObjectRef::new(lua, descendant)));
                }
            }
            Ok(None)
        }
        fn get_actor(lua: &Lua, this: ObjectRef) -> LuaResult<Option<ObjectRef>> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            Ok(world.get::<ContainerProvenance>(this.entity()).map(|p| ObjectRef::new(lua, p.0)))
        }
        fn get_attribute(lua: &Lua, this: ObjectRef, key: String) -> LuaResult<LuaValue> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();

            world.get::<InstanceMembers>(this.entity()).expect("this is an instance")
                .attributes
                .get(&key).cloned()
                .unwrap_or_default()
                .into_lua(lua)
        }
        fn get_attribute_changed_signal(lua: &Lua, this: ObjectRef, key: String) -> LuaResult<LuaValue> {
            let mut world_access = WorldAccess::fetch(lua);
            let world = world_access.access_synchronized()?;

            world.get_mut::<InstanceMembers>(this.entity()).expect("this is an instance")
                .attribute_changed_signal
                .entry(key).or_default()
                .into_lua(lua)
        }
        fn get_attributes(lua: &Lua, this: ObjectRef) -> LuaResult<LuaValue> {
            let world_access = WorldAccess::fetch_readonly(lua);
            let world = world_access.access_read_only();

            let table = lua.create_table()?;

            for (k, v) in world.get::<InstanceMembers>(this.entity()).expect("this is an instance").attributes.iter() {
                table.raw_set(k.as_str(), v)?;
            }
            table.into_lua(lua)
        }
        #[deprecated_alias="getChildren"]
        fn get_children(lua: &Lua, this: ObjectRef) -> LuaResult<Vec<ObjectRef>> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();

            Ok(world.get::<Children>(this.entity())
                .expect("instance has children")
                .iter()
                .map(|e| ObjectRef::new(lua, e))
                .collect::<Vec<_>>()
            )
        }
        fn get_debug_id(lua: &Lua, this: ObjectRef, scope_len: u64) -> LuaResult<String> {
            lua_todo!()
        }
        fn get_descendants(lua: &Lua, this: ObjectRef) -> LuaResult<Vec<ObjectRef>> {
            lua_todo!()
        }
        fn get_full_name(lua: &Lua, this: ObjectRef) -> LuaResult<String> {
            lua_todo!()
        }
        fn get_styled(lua: &Lua, this: ObjectRef, name: String, selector: Option<String>) -> LuaResult<LuaValue> {
            lua_todo!()
        }
        fn get_styled_property_changed_signal(lua: &Lua, this: ObjectRef, property: String) -> LuaResult<LuaValue> {
            lua_todo!()
        }
        fn get_tags(lua: &Lua, this: ObjectRef) -> LuaResult<LuaValue> {
            let cs = {
                let wa = WorldAccess::fetch_readonly(lua);
                let world = wa.access_read_only();
                
                let mut root_instance_qs = world.try_query_filtered::<Entity, With<RootInstance>>().unwrap();
                let game_instance = root_instance_qs.single(&*world).expect("single game instance");

                let mut objects_qs = world.try_query::<&ObjectHeader>().unwrap();
                let children = world.get::<Children>(game_instance).unwrap();
                children.iter().filter(|e| {
                    let header = objects_qs.get(&*world, *e).expect("expected object");

                    header.vtable.class_name == "CollectionService"
                }).next().expect("collection service exists")
            };
            CollectionService::get_tags(lua, (ObjectRef::new(lua, cs), this))
        }
        fn has_tag(lua: &Lua, this: ObjectRef, tag: String) -> LuaResult<bool> {
            let cs = {
                let wa = WorldAccess::fetch_readonly(lua);
                let world = wa.access_read_only();
                
                let mut root_instance_qs = world.try_query_filtered::<Entity, With<RootInstance>>().unwrap();
                let game_instance = root_instance_qs.single(&*world).expect("single game instance");

                let mut objects_qs = world.try_query::<&ObjectHeader>().unwrap();
                let children = world.get::<Children>(game_instance).unwrap();
                children.iter().filter(|e| {
                    let header = objects_qs.get(&*world, *e).expect("expected object");

                    header.vtable.class_name == "CollectionService"
                }).next().expect("collection service exists")
            };
            CollectionService::has_tag(lua, (ObjectRef::new(lua, cs), this, tag))
        }
        fn is_ancestor_of(lua: &Lua, this: ObjectRef, descendant: ObjectRef) -> LuaResult<bool> {
            lua_todo!()
        }
        #[deprecated_alias="isDescendantOf"]
        fn is_descendant_of(lua: &Lua, this: ObjectRef, ancestor: ObjectRef) -> LuaResult<bool> {
            lua_todo!()
        }
        fn is_property_modified(lua: &Lua, this: ObjectRef, property: String) -> LuaResult<bool> {
            lua_todo!()
        }
        fn query_descendants(lua: &Lua, this: ObjectRef, selector: String) -> LuaResult<LuaValue> {
            lua_todo!()
        }

        // fn remove(lua: &Lua, this: ObjectRef) ->
        fn remove_tag(lua: &Lua, this: ObjectRef, tag: String) -> LuaResult<()> {
            let cs = {
                let mut wa = WorldAccess::fetch(lua);
                let world = wa.access_synchronized()?;
                
                let mut root_instance_qs = world.query_filtered::<Entity, With<RootInstance>>();
                let game_instance = root_instance_qs.single(world).expect("single game instance");

                let mut objects_qs = world.query::<&ObjectHeader>();
                let children = world.get::<Children>(game_instance).unwrap();
                children.iter().filter(|e| {
                    let header = objects_qs.get(world, *e).expect("expected object");

                    header.vtable.class_name == "CollectionService"
                }).next().expect("collection service exists")
            };
            CollectionService::remove_tag(lua, (ObjectRef::new(lua, cs), this, tag))
        }
        fn reset_property_to_default(lua: &Lua, this: ObjectRef, property: String) -> LuaResult<()> {
            lua_todo!()
        }
        fn set_attribute(lua: &Lua, this: ObjectRef, attribute: String, value: LuaValue) -> LuaResult<()> {
            lua_todo!()
        }
    }
}
