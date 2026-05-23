use std::mem::take;

use crate::core::luau::ContainerProvenance;
use crate::enums::MessageType;
use crate::userdata::{LuaFreeValue, RBXScriptSignal};
use crate::{core::WorldAccess, internal_prelude::*, userdata::ObjectRef};
use bevy::ecs::entity::{EntityCloner, EntityHashMap};
use bevy::prelude::*;

use bevy::{
    ecs::{component::Component, hierarchy::Children, name::Name, system::EntityCommands},
    platform::collections::HashMap,
};
use bevy_rblx_derive::register_class;
use lazy_static::lazy_static;
use mlua::prelude::*;

use crate::core::object::ObjectNewFn;
use crate::core::{
    CollectionService, DisabledObject, ObjectHeader, ObjectVTableCreationPointer, push_log,
};

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

            let e = entity.id();
            entity.commands_mut().write_message(NewInstanceEvent(e));
            Ok(())
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
            let e = entity.id();

            entity.commands_mut().write_message(NewInstanceEvent(e));
            Ok(())
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

        let mut ancestors_qs = world.query_filtered::<&ChildOf, Allow<DisabledObject>>();
        let ancestors = ancestors_qs.query(&*world);
        let parent = ancestors.get(this.entity()).ok().map(|x| x.0);
        if parent.is_none() {
            return Ok(());
        }

        for a in ancestors.iter_ancestors(this.entity()) {
            let ancestor_members = InstanceMembers::fetch_members(&*world, a);
            events.push(ancestor_members.descendant_removing.reference());
        }
    }
    for ev in take(&mut events) {
        ev.fire_in_lua(lua, true, ObjectRef::new(lua, this))?
    }

    let child_removed = {
        let mut wa = WorldAccess::fetch(lua);
        let world = wa.access_synchronized()?;
        let parent = world.get::<ChildOf>(this).unwrap().0;
        world.entity_mut(parent).remove::<ChildOf>();

        let mut descendants_qs = world.query_filtered::<&Children, Allow<DisabledObject>>();
        let descendants = descendants_qs.query(world);

        for descendant in descendants.iter_descendants(this) {
            let descendant_members = InstanceMembers::fetch_members(&*world, descendant);
            events.push(descendant_members.ancestry_changed.reference());
        }

        let parent_members = InstanceMembers::fetch_members(&*world, parent);
        parent_members.child_removed.reference()
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
        let mut ancestors_qs = world.query_filtered::<&ChildOf, Allow<DisabledObject>>();
        let ancestors = ancestors_qs.query(&*world);
        events.push(
            InstanceMembers::fetch_members(world, this)
                .child_added
                .reference(),
        );
        for ancestor in ancestors.iter_ancestors(this) {
            events.push(
                InstanceMembers::fetch_members(world, ancestor)
                    .descendant_added
                    .reference(),
            )
        }
    }

    Ok(())
}

impl Instance {
    pub fn force_set_parent(lua: &Lua, this: Entity, new_parent: Option<Entity>) -> LuaResult<()> {
        let vtable;
        let has_parent = {
            let mut wa = WorldAccess::fetch(lua);
            wa.assert_synchronized()?;
            let world = wa.access_read_only();
            vtable = world.get::<ObjectHeader>(this).unwrap().vtable;
            world.get::<ChildOf>(this).is_some()
        };
        if has_parent {
            remove_parent(lua, this, new_parent)?;
        }
        if let Some(new_parent) = new_parent {
            add_parent(lua, this, new_parent)?;
        }
        match vtable.lazy_full_fields.get("Parent").unwrap() {
            super::object::ObjectField::Property(object_property_info) => {
                object_property_info.fire_changed_event(lua, this, vtable)?
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}

register_class! {
    #[post_init=fn (lua: &Lua, this: Entity) -> LuaResult<()> {
        let mut wa = WorldAccess::fetch(lua);
        let world = wa.access_synchronized()?;
        let vtable = world.get::<ObjectHeader>(this).unwrap().vtable;
        {
            let mut name = world.get_mut::<Name>(this).unwrap();
            if vtable.class_name == "RunService" {
                name.set("Run Service");
            } else {
                name.set(vtable.class_name);
            }
        }
        Ok(())

    }]
    #[custom_getter=fn(lua: &Lua, this: Entity, field: &str) -> LuaResult<LuaValue> {
        let entity;
        'outer: {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            for e in world.get::<Children>(this).unwrap() {
                if world.get::<Name>(*e).unwrap().as_str() == field {
                    entity = *e;
                    break 'outer;
                }
            }
            return Ok(LuaValue::Nil)
        }
        ObjectRef::new(lua, entity).into_lua(lua)
    }]
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
            if diff {
                name.set(new_name);
            }
            Ok(diff)
        }]
        #[deprecated_alias="name"]
        virtual name: String,

        #[getter=fn(lua: &Lua, this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let e = {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();

                if let Some(parent) = world.get::<ChildOf>(this) {
                    parent.0
                } else {
                    return Ok(LuaValue::Nil);
                }
            };
            ObjectRef::new(lua, e).into_lua(lua)
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
                    drop(world);
                    drop(wa);
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

                let mut objects_qs = world.query_filtered::<&ObjectHeader, Allow<DisabledObject>>();
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
                let new_instance = world.spawn(Children::default()).id();
                let mut cloner = {
                    let mut builder = EntityCloner::build_opt_out(world);
                    builder
                        .linked_cloning(false)
                        .deny::<(ChildOf, Children)>();
                    builder.finish()
                };
                let mut descendants_qs = world.query_filtered::<&Children, Allow<DisabledObject>>();
                let mut ancestors_qs = world.query_filtered::<&ChildOf, Allow<DisabledObject>>();
                let mut mapper = EntityHashMap::<Entity>::default();
                let parent_and_child = descendants_qs.query(world)
                    .iter_descendants(this.entity())
                    .map(|child| (ancestors_qs.get(world, child).unwrap().0, child))
                    .collect::<Vec<_>>();
                cloner.clone_entity(world, this.entity(), new_instance);
                mapper.insert(this.entity(), new_instance);
                for (parent, child) in parent_and_child {
                    if let Some(i) = world.get::<InstanceMembers>(child) {
                        if !i.archivable || i.cloning_protected || mapper.get(&parent).is_none() {
                            continue;
                        }
                    } else {
                        continue;
                    }
                    let new_child = world.spawn((Children::default(), ChildOf(*mapper.get(&parent).unwrap()))).id();
                    cloner.clone_entity(world, child, new_child);
                }

                let all_instances = std::iter::once(new_instance).chain(world.query_filtered::<&Children, Allow<DisabledObject>>().query(world).iter_descendants(new_instance)).map(|x| NewInstanceEvent(x)).collect::<Vec<_>>();
                world.write_message_batch(all_instances);
                new_instance
            };
            Ok(Some(ObjectRef::new(lua, new_instance)))
        }
        #[deprecated_alias="destroy"]
        fn destroy(lua: &Lua, this: ObjectRef) -> LuaResult<()> {
            let (vtable, destroying) = {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();
                let members = world.get::<InstanceMembers>(this.entity()).expect("is instance");
                if members.destroy_protected {
                    drop(world);
                    drop(world_access);
                    push_log(lua, MessageType::MessageWarning, "object is not destroyable");
                    return Ok(());
                }
                if members.destroyed {
                    drop(world);
                    drop(world_access);
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
            let mut e = None;
            {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();
                let mut query_state = world.try_query_filtered::<&ChildOf, Allow<DisabledObject>>().expect("query state was not initialized :(");
                let query = query_state.query(&*world);
                for ancestor in query.iter_ancestors(this.entity()) {
                    if world.get::<Name>(ancestor).expect("expecting instance").as_str() == name {
                        e = Some(ancestor);
                        break;
                    }
                }
            }
            Ok(e.map(|x| ObjectRef::new(lua, x)))
        }
        fn find_first_ancestor_of_class(lua: &Lua, this: ObjectRef, class_name: String) -> LuaResult<Option<ObjectRef>> {
            let mut e = None;
            {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();
                let mut query_state = world.try_query_filtered::<&ChildOf, Allow<DisabledObject>>().expect("query state was not initialized :(");
                let query = query_state.query(&*world);
                for ancestor in query.iter_ancestors(this.entity()) {
                    if world.get::<ObjectHeader>(ancestor).expect("expecting object").vtable.class_name == &class_name {
                        e = Some(ancestor);
                        break;

                    }
                }
            }
            Ok(e.map(|x| ObjectRef::new(lua, x)))
        }
        fn find_first_ancestor_which_is_a(lua: &Lua, this: ObjectRef, class_name: String) -> LuaResult<Option<ObjectRef>> {
            let mut e = None;
            {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();
                let mut query_state = world.try_query_filtered::<&ChildOf, Allow<DisabledObject>>().expect("query state was not initialized :(");
                let query = query_state.query(&*world);
                for ancestor in query.iter_ancestors(this.entity()) {
                    for i in world.get::<ObjectHeader>(ancestor).expect("expecting object").vtable.method_resolution_order.iter() {
                        if i.class_name == &class_name {
                            e = Some(ancestor);
                            break;
                        }
                    }
                }
            }
            Ok(e.map(|x| ObjectRef::new(lua, x)))
        }
        #[deprecated_alias="findFirstChild"]
        fn find_first_child(lua: &Lua, this: ObjectRef, name: String) -> LuaResult<Option<ObjectRef>> {
            let mut e = None;
            {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();
                for child in world.get::<Children>(this.entity()).expect("instances have children").iter() {
                    if world.get::<Name>(child).expect("expecting instance").as_str() == name {
                        e = Some(child);
                        break;
                    }
                }
            }
            Ok(e.map(|x| ObjectRef::new(lua, x)))
        }
        fn find_first_child_of_class(lua: &Lua, this: ObjectRef, class_name: String) -> LuaResult<Option<ObjectRef>> {
            let mut e = None;
            {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();
                for child in world.get::<Children>(this.entity()).expect("instances have children").iter() {
                    let cn = world.get::<ObjectHeader>(child).expect("expecting object").vtable.class_name;
                    if cn == &class_name {
                        e = Some(child);
                        break;
                    }
                }
            }
            Ok(e.map(|x| ObjectRef::new(lua, x)))
        }
        fn find_first_child_which_is_a(lua: &Lua, this: ObjectRef, class_name: String) -> LuaResult<Option<ObjectRef>> {
            let mut e = None;
            {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();
                for child in world.get::<Children>(this.entity()).expect("instances have children").iter() {
                    for i in world.get::<ObjectHeader>(child).expect("expecting object").vtable.method_resolution_order.iter() {
                        if i.class_name == &class_name {
                            e = Some(child);
                            break;
                        }
                    }
                }
            }
            Ok(e.map(|x| ObjectRef::new(lua, x)))
        }
        fn find_first_descendant(lua: &Lua, this: ObjectRef, name: String) -> LuaResult<Option<ObjectRef>> {
            let mut e = None;
            {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();
                let mut query_state = world.try_query_filtered::<&Children, Allow<DisabledObject>>().expect("query state was not initialized :(");
                let query = query_state.query(&*world);
                for descendant in query.iter_descendants(this.entity()) {
                    if world.get::<Name>(descendant).expect("expecting instance").as_str() == name {
                        e = Some(descendant);
                        break;
                    }
                }
            }
            Ok(e.map(|x| ObjectRef::new(lua, x)))
        }
        fn get_actor(lua: &Lua, this: ObjectRef) -> LuaResult<Option<ObjectRef>> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            let new_entity = world.get::<ContainerProvenance>(this.entity()).filter(|x| world.get::<RootInstance>(x.0).is_none());
            Ok(new_entity.map(|p| ObjectRef::new(lua, p.0)))
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
                .entry(key).or_default().reference()
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
            let entities = {
                let wa = WorldAccess::fetch_readonly(lua);
                let world = wa.access_read_only();
                world.get::<Children>(this.entity())
                    .expect("instance has children")
                    .iter().collect::<Vec<_>>()
            };
            Ok(
                entities.into_iter().map(|e| ObjectRef::new(lua, e))
                .collect::<Vec<_>>()
            )
        }
        fn get_debug_id(lua: &Lua, this: ObjectRef, scope_len: u64) -> LuaResult<String> {
            lua_todo!()
        }
        fn get_descendants(lua: &Lua, this: ObjectRef) -> LuaResult<Vec<ObjectRef>> {
            let v = {
                let wa = WorldAccess::fetch_readonly(lua);
                let world = wa.access_read_only();

                let mut children_qs = world.try_query_filtered::<&Children, Allow<DisabledObject>>().expect("failed to create children query");
                let children = children_qs.query(&*world);
                children.iter_descendants(this.entity()).collect::<Vec<_>>()
            };
            Ok(v.into_iter().map(|x| ObjectRef::new(lua, x)).collect())
        }
        fn get_full_name(lua: &Lua, this: ObjectRef) -> LuaResult<String> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();

            let mut ancestors_qs = world.try_query_filtered::<&ChildOf, Allow<DisabledObject>>().expect("failed to create ancestors query");
            let ancestors = ancestors_qs.query(&*world).iter_ancestors(this.entity()).collect::<Vec<_>>();

            match ancestors.len() {
                0 | 1 => Ok(world.get::<Name>(this.entity()).unwrap().to_string()),
                _ => {
                    let v = ancestors.into_iter().skip(1).map(|x| world.get::<Name>(x).unwrap().as_str()).collect::<Vec<_>>();
                    Ok(v.join("."))
                }
            }
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
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();

            let mut p = world.get::<ChildOf>(descendant.entity()).map(|x| x.0);
            while let Some(parent) = p {
                if parent == this.entity() {
                    return Ok(true);
                }
                p = world.get::<ChildOf>(parent).map(|x| x.0);
            }
            Ok(false)
        }
        #[deprecated_alias="isDescendantOf"]
        fn is_descendant_of(lua: &Lua, this: ObjectRef, ancestor: ObjectRef) -> LuaResult<bool> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();

            let mut p = world.get::<ChildOf>(this.entity()).map(|x| x.0);
            while let Some(parent) = p {
                if parent == ancestor.entity() {
                    return Ok(true);
                }
                p = world.get::<ChildOf>(parent).map(|x| x.0);
            }
            Ok(false)
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
            let new_value = LuaFreeValue::from_lua(value, lua)?;
            let attrib_changed;
            let attrib_self_changed;
            {
                let mut wa = WorldAccess::fetch(lua);
                let world = wa.access_synchronized()?;

                let mut members = InstanceMembers::fetch_members_mut(world, this.entity());

                attrib_changed = members.attribute_changed.reference();
                attrib_self_changed = members.attribute_changed_signal.get(&attribute).map(|x| x.reference());
                members.attributes.insert(attribute.clone(), new_value.clone());
            }
            attrib_changed.fire_in_lua(lua, false, attribute)?;
            if let Some(ev) = attrib_self_changed {
                ev.fire_in_lua(lua, false, new_value)?;
            }
            Ok(())
        }
        fn debug_print_tree(lua: &Lua, this: ObjectRef) -> LuaResult<()> {
            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;

            let mut names_qs = world.query_filtered::<&Name, Allow<DisabledObject>>();
            let mut descendants_qs = world.query_filtered::<&Children, Allow<DisabledObject>>();
            let mut objects_qs = world.query_filtered::<&ObjectHeader, Allow<DisabledObject>>();

            let descendants = descendants_qs.query(world);
            let names = names_qs.query(world);
            let objects = objects_qs.query(world);

            let mut tree_build = ptree::TreeBuilder::new(format!("{} ({}) id: {}", names.get(this.entity()).unwrap(), objects.get(this.entity()).unwrap().vtable.class_name, this.entity()));
            let mut stack = Vec::new();
            stack.push(descendants.get(this.entity()).unwrap().len());
            for e in descendants.iter_descendants_depth_first(this.entity()) {
                *stack.last_mut().unwrap() -= 1;
                let c = descendants.get(e).unwrap();
                let name = format!("{} ({}) id: {}", names.get(e).unwrap(), objects.get(e).unwrap().vtable.class_name, e);
                if c.is_empty() {
                    tree_build.add_empty_child(name);
                } else {
                    tree_build.begin_child(name);
                    stack.push(c.len());
                }
                while let Some(x) = stack.last().copied() && x == 0 {
                    stack.pop();
                }
            }
            ptree::print_tree(&tree_build.build()).into_lua_err()?;
            Ok(())
        }
    }
}
