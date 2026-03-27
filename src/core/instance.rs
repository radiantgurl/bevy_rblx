use bevy::prelude::*;
use bevy::{
    ecs::{component::Component, hierarchy::Children, name::Name, system::EntityCommands},
    platform::collections::HashMap,
};
use lazy_static::lazy_static;
use mlua::{Error, Lua, Value, prelude::LuaResult};

use crate::core::object::ObjectNewFn;
use crate::core::object::ObjectVTable;

#[derive(Component, Clone)]
#[require(Name, Children)]
pub struct InstanceHeader {
    pub archivable: bool,
    pub attributes: HashMap<String, Value>,
    pub destroyed: bool,
    pub allow_cloning: bool,
    pub destroy_protected: bool,
    pub parent_protected: bool,
}

impl Default for InstanceHeader {
    fn default() -> Self {
        Self {
            archivable: true,
            attributes: HashMap::default(),
            destroyed: false,
            allow_cloning: true,
            destroy_protected: false,
            parent_protected: false,
        }
    }
}

impl InstanceHeader {
    pub fn protected() -> Self {
        Self {
            allow_cloning: false,
            destroy_protected: true,
            parent_protected: true,
            ..default()
        }
    }
}
pub struct InstanceConstructor {
    visible: HashMap<&'static str, fn(&Lua, EntityCommands) -> LuaResult<()>>,
    all: HashMap<&'static str, fn(&Lua, EntityCommands) -> LuaResult<()>>,
}

lazy_static! {
    pub static ref INSTANCE_CONSTRUCTOR: InstanceConstructor = {
        let all = inventory::iter::<ObjectVTable>
            .into_iter()
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
        let visible = inventory::iter::<ObjectVTable>
            .into_iter()
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
            Err(Error::runtime(format!(
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
            Err(Error::runtime(format!(
                "Cannot construct instance of type {class_name}"
            )))
        }
    }
}

#[derive(Clone, Copy, Component, Debug)]
pub struct ActorProvenance(pub Entity);
