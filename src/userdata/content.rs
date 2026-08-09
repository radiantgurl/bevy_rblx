use crate::{
    core::{lua::LuaSingleton, push_log},
    enums::{ContentSourceType, MessageType},
    internal_prelude::*,
};
use bevy_rblx_derive::register;
use mlua::prelude::*;

use crate::userdata::ObjectRef;

#[derive(Default, Clone, PartialEq, Eq, Hash, FromLua, Debug)]
pub enum Content {
    #[default]
    None,
    Uri(String),
    ObjectRef(ObjectRef),
}

impl Content {
    pub fn from_uri(uri: String) -> LuaResult<Content> {
        if uri.starts_with("rbxgameasset://") || uri.starts_with("rbxassetid://") {
            Ok(Content::Uri(uri))
        } else {
            Err(LuaError::runtime("Invalid uri provided."))
        }
    }
    pub fn from_object(object_ref: ObjectRef) -> Self {
        Self::ObjectRef(object_ref)
    }
    pub fn from_asset_id(asset_id: u64) -> Self {
        Self::Uri(format!("rbxassetid://{asset_id}"))
    }

    fn from_uri_lua(lua: &Lua, uri: String) -> LuaResult<Content> {
        match Self::from_uri(uri) {
            Ok(c) => Ok(c),
            Err(_) => {
                let t = lua
                    .traceback(Some("Invalid URI provided."), 2)?
                    .to_string_lossy();
                push_log(lua, MessageType::MessageWarning, t);
                Ok(Content::None)
            }
        }
    }
}

impl LuaUserData for Content {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("SourceType", move |_, o| {
            Ok(match o {
                Content::None => ContentSourceType::None,
                Content::Uri(_) => ContentSourceType::Uri,
                Content::ObjectRef(_) => ContentSourceType::Object,
            })
        });
        fields.add_field_method_get("Uri", move |lua, o| {
            Ok(match o {
                Content::Uri(v) => v.as_str().into_lua(lua)?,
                _ => LuaValue::Nil,
            })
        });
        fields.add_field_method_get("Object", move |lua, o| {
            Ok(match o {
                Content::ObjectRef(v) => v.clone().into_lua(lua)?,
                _ => LuaValue::Nil,
            })
        });
    }
}

#[register]
impl LuaSingleton for Content {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        let t = lua.create_table()?;
        t.raw_set("none", Content::None)?;
        t.raw_set("fromUri", lua.create_function(Content::from_uri_lua)?)?;
        t.raw_set(
            "fromObject",
            lua.create_function(move |_, o: ObjectRef| Ok(Content::from_object(o)))?,
        )?;
        t.raw_set(
            "fromAssetId",
            lua.create_function(move |_, id: u64| Ok(Content::from_asset_id(id)))?,
        )?;
        t.set_readonly(true);
        lua.globals().raw_set("Content", t)
    }
}
