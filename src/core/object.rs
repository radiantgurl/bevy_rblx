use std::{mem::take, sync::LazyLock};

use crate::{
    core::{ThreadIdentity, WorldAccess},
    internal_prelude::*,
    userdata::{LuaFreeValue, ObjectRef, RBXScriptSignal},
};

use bevy::{platform::collections::HashMap, prelude::*};
use bevy_rblx_derive::register;
use lazy_static::lazy_static;
use mlua::prelude::*;

use crate::core::{LuaSingleton, RefCounted, SecurityContext};

#[derive(Component, Clone, Debug)]
#[require(RefCounted)]
pub struct ObjectHeader {
    pub vtable: &'static ObjectVTable,
    property_changed: HashMap<String, RBXScriptSignal>,
    pub changed: RBXScriptSignal,
}

impl ObjectHeader {
    pub fn new(vtable: &'static ObjectVTable) -> Self {
        Self {
            vtable,
            property_changed: HashMap::default(),
            changed: RBXScriptSignal::default(),
        }
    }
}

#[derive(Debug)]
pub enum ObjectNewFn {
    None,
    Protected(fn(&Lua, EntityCommands) -> LuaResult<()>),
    Visible(fn(&Lua, EntityCommands) -> LuaResult<()>),
}

pub type LuaObjectGetterFn = fn(&Lua, Entity, &'static ObjectVTable) -> LuaResult<LuaValue>;
pub type LuaObjectSetterFn = fn(&Lua, Entity, &'static ObjectVTable, LuaValue) -> LuaResult<bool>;

#[derive(Debug)]
pub struct ObjectPropertyInfo {
    pub property_name: &'static str,
    pub security: SecurityContext,

    pub getter: LuaObjectGetterFn,
    pub setter: Option<LuaObjectSetterFn>,
    #[cfg(feature = "deprecated")]
    pub deprecated_alias_of: Option<&'static str>,
    #[cfg(feature = "deprecated")]
    pub deprecated_aliases: Vec<&'static str>,
}

impl ObjectPropertyInfo {
    pub fn fire_changed_event(
        &'static self,
        lua: &Lua,
        object: Entity,
        vtable: &'static ObjectVTable,
    ) -> LuaResult<()> {
        let new_value = LuaFreeValue::from_lua((self.getter)(lua, object, vtable)?, lua)?;
        let mut res;
        #[cfg(not(feature = "deprecated"))]
        {
            let changed;
            let property_changed_signal;
            {
                let world_access = WorldAccess::fetch_readonly(lua);
                let world = world_access.access_read_only();
                let header = world.get::<ObjectHeader>(object).expect("entity is object");
                changed = header.changed.reference();
                if let Some(ev) = header.property_changed.get(self.property_name) {
                    property_changed_signal = Some(ev.reference());
                } else {
                    property_changed_signal = None;
                }
            }
            res = changed.fire_in_lua(lua, self.property_name == "Parent", self.property_name);
            if let Some(property_changed) = property_changed_signal {
                res = res.and(property_changed.fire_in_lua(
                    lua,
                    self.property_name == "Parent",
                    new_value,
                ));
            }
        }
        #[cfg(feature = "deprecated")]
        {
            todo!()
        }
        res
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CachedLuaFunction(fn(&Lua) -> LuaFunction);

impl CachedLuaFunction {
    pub fn fetch(&self, lua: &Lua) -> LuaFunction {
        lua.app_data_mut::<CachedLuaFunctions>()
            .unwrap()
            .0
            .entry(self.0 as usize)
            .or_insert_with(|| self.0(lua))
            .clone()
    }
    pub const fn new(generator: fn(&Lua) -> LuaFunction) -> Self {
        Self(generator)
    }
}

#[derive(Default)]
struct CachedLuaFunctions(HashMap<usize, LuaFunction>);

#[register]
impl LuaSingleton for CachedLuaFunctions {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.set_app_data(CachedLuaFunctions::default());
        Ok(())
    }
}

#[derive(Debug)]
pub struct ObjectMethodInfo {
    pub method_name: &'static str,
    pub security: SecurityContext,

    pub function: CachedLuaFunction,
}

#[derive(Debug, Clone, Copy)]
pub enum ObjectField {
    Method(&'static ObjectMethodInfo),
    Property(&'static ObjectPropertyInfo),
}

impl ObjectField {
    pub const fn security_context(&self) -> SecurityContext {
        match self {
            ObjectField::Method(object_method_info) => object_method_info.security,
            ObjectField::Property(object_property_info) => object_property_info.security,
        }
    }
    pub const fn name(&self) -> &'static str {
        match self {
            ObjectField::Method(object_method_info) => object_method_info.method_name,
            ObjectField::Property(object_property_info) => object_property_info.property_name,
        }
    }
    pub fn get(
        &self,
        lua: &Lua,
        object: Entity,
        vtable: &'static ObjectVTable,
    ) -> LuaResult<LuaValue> {
        let current_context = ThreadIdentity::fetch(lua).identity.get_security_contexts();
        let expected_context = self.security_context();
        if !current_context.has(expected_context) {
            return Err(LuaError::runtime(format!(
                "thread with security context {current_context} is missing {expected_context}"
            )));
        }
        match self {
            ObjectField::Method(object_method_info) => {
                object_method_info.function.fetch(lua).into_lua(lua)
            }
            ObjectField::Property(object_property_info) => {
                (object_property_info.getter)(lua, object, vtable)
            }
        }
    }
    pub fn set(
        &self,
        lua: &Lua,
        object: Entity,
        vtable: &'static ObjectVTable,
        value: LuaValue,
    ) -> LuaResult<()> {
        let current_context = ThreadIdentity::fetch(lua).identity.get_security_contexts();
        let expected_context = self.security_context();
        if !current_context.has(expected_context) {
            return Err(LuaError::runtime(format!(
                "thread with security context {current_context} is missing {expected_context}"
            )));
        }
        match self {
            ObjectField::Property(object_property_info) => {
                if let Some(setter) = object_property_info.setter {
                    if setter(lua, object, vtable, value)? {
                        object_property_info.fire_changed_event(lua, object, vtable)?;
                    }
                    return Ok(());
                }
            }
            _ => (),
        }
        Err(LuaError::runtime(format!(
            "property {} is not settable",
            self.name()
        )))
    }
}
// Implementation detail: method_resolution_order contains self as the first, and at last is the Object root class.
#[derive(Debug)]
pub struct ObjectVTable {
    pub class_name: &'static str,
    pub inherits: &'static [&'static str],

    pub properties: &'static [ObjectPropertyInfo],
    pub methods: &'static [ObjectMethodInfo],

    pub new: ObjectNewFn,

    pub method_resolution_order: LazyLock<Vec<&'static ObjectVTable>>,
    pub lazy_full_fields: LazyLock<HashMap<&'static str, ObjectField>>,
}

#[doc(hidden)]
pub struct ObjectVTableCreationPointer(pub fn() -> &'static ObjectVTable);
inventory::collect!(ObjectVTableCreationPointer);

lazy_static! {
    pub static ref OBJECT_VTABLES: HashMap<&'static str, &'static ObjectVTable> =
        inventory::iter::<ObjectVTableCreationPointer>
            .into_iter()
            .map(|x| x.0())
            .map(|x| (x.class_name, x))
            .collect();
}

impl ObjectVTable {
    #[doc(hidden)]
    pub fn fetch_full_fields(class_name: &'static str) -> HashMap<&'static str, ObjectField> {
        let vtable = OBJECT_VTABLES.get(class_name).unwrap();
        let mut fields = HashMap::new();

        for cls_vtable in vtable.method_resolution_order.iter() {
            for prop in cls_vtable.properties.iter() {
                fields
                    .entry(prop.property_name)
                    .or_insert(ObjectField::Property(prop));
            }
            for method in cls_vtable.methods.iter() {
                fields
                    .entry(method.method_name)
                    .or_insert(ObjectField::Method(method));
            }
        }

        fields
    }
    #[doc(hidden)]
    pub fn generate_method_resolution_order(
        class_name: &'static str,
    ) -> Vec<&'static ObjectVTable> {
        let mut mro: Vec<&'static str> = Vec::default();
        let mut recursion: Vec<&'static str> = vec![class_name];

        while recursion.len() != 0 {
            for i in take(&mut recursion) {
                let vtable = OBJECT_VTABLES.get(i).expect("class exists");
                if let Some((idx, _)) = mro.iter().copied().enumerate().find(|(_, v)| *v == i) {
                    mro.remove(idx);
                }
                mro.push(i);
                recursion.extend(vtable.inherits);
            }
        }

        assert!(
            *mro.last().unwrap() == "Object",
            "invalid method resolution order for {class_name}"
        );

        mro.into_iter()
            .map(|x| *OBJECT_VTABLES.get(&x).expect("class exists"))
            .collect()
    }
}

const _: () = {
    fn class_name_getter(
        lua: &Lua,
        _: Entity,
        vtable: &'static ObjectVTable,
    ) -> LuaResult<LuaValue> {
        vtable.class_name.into_lua(lua)
    }
    fn changed_getter(lua: &Lua, e: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
        (&WorldAccess::fetch_readonly(lua)
            .access_read_only()
            .get::<ObjectHeader>(e)
            .unwrap()
            .changed)
            .into_lua(lua)
    }

    fn get_property_changed_signal(
        lua: &Lua,
        (o, key): (ObjectRef, String),
    ) -> LuaResult<LuaValue> {
        let mut world_access = WorldAccess::fetch(lua);
        let world = world_access.access_synchronized()?;
        let mut obj = world.get_mut::<ObjectHeader>(o.entity()).unwrap();
        if let Some(v) = obj.property_changed.get(&key) {
            v.into_lua(lua)
        } else {
            obj.property_changed.entry(key).or_default().into_lua(lua)
        }
    }
    fn is_a(lua: &Lua, (o, class): (ObjectRef, String)) -> LuaResult<LuaValue> {
        let world_access = WorldAccess::fetch_readonly(lua);
        let world = world_access.access_read_only();
        world
            .get::<ObjectHeader>(o.entity())
            .unwrap()
            .vtable
            .method_resolution_order
            .iter()
            .any(|x| x.class_name == class)
            .into_lua(lua)
    }

    static VTABLE: ObjectVTable = ObjectVTable {
        class_name: "Object",
        inherits: &[],
        properties: &[
            ObjectPropertyInfo {
                property_name: "ClassName",
                security: SecurityContext::NONE,
                getter: class_name_getter,
                setter: None,
            },
            #[cfg(feature = "deprecated")]
            ObjectPropertyInfo {
                property_name: "className",
                security: SecurityContext::NONE,
                getter: class_name_getter,
                setter: None,
            },
            ObjectPropertyInfo {
                property_name: "Changed",
                security: SecurityContext::NONE,
                getter: changed_getter,
                setter: None,
            },
        ],
        methods: &[
            ObjectMethodInfo {
                method_name: "GetPropertyChangedSignal",
                security: SecurityContext::NONE,
                function: CachedLuaFunction::new(move |lua: &Lua| {
                    lua.create_function(get_property_changed_signal)
                        .expect("no error")
                }),
            },
            ObjectMethodInfo {
                method_name: "IsA",
                security: SecurityContext::NONE,
                function: CachedLuaFunction::new(move |lua: &Lua| {
                    lua.create_function(is_a).expect("no error")
                }),
            },
            #[cfg(feature = "deprecated")]
            ObjectMethodInfo {
                method_name: "isA",
                security: SecurityContext::NONE,
                function: CachedLuaFunction::new(move |lua: &Lua| {
                    lua.create_function(is_a).expect("no error")
                }),
            },
        ],
        new: ObjectNewFn::None,
        method_resolution_order: LazyLock::new(move || {
            ObjectVTable::generate_method_resolution_order("Object")
        }),
        lazy_full_fields: LazyLock::new(move || ObjectVTable::fetch_full_fields("Object")),
    };

    inventory::submit!(ObjectVTableCreationPointer(move || &VTABLE));
};

impl ObjectVTable {
    pub fn get(&'static self, lua: &Lua, object: Entity, index: String) -> LuaResult<LuaValue> {
        if let Some(field) = self.lazy_full_fields.get(index.as_str()) {
            field.get(lua, object, self)
        } else {
            Err(LuaError::runtime(format!(
                "object {} has no property {index}",
                self.class_name
            )))
        }
    }
    pub fn set(
        &'static self,
        lua: &Lua,
        object: Entity,
        index: String,
        value: LuaValue,
    ) -> LuaResult<()> {
        if let Some(field) = self.lazy_full_fields.get(index.as_str()) {
            field.set(lua, object, self, value)
        } else {
            Err(LuaError::runtime(format!(
                "object {} has no property {index}",
                self.class_name
            )))
        }
    }
    pub fn is_a(&'static self, class_name: &str) -> bool {
        for vtable in self.method_resolution_order.iter() {
            if vtable.class_name == class_name {
                return true;
            }
        }
        false
    }
}
