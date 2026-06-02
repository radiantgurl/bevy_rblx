use std::mem::take;

use crate::core::lua::{LuaSingleton, WorldAccess, system_time};
use crate::core::object::ServiceMembers;
use crate::core::object::service::DisablingService;
use crate::enums::MessageType;
use crate::internal_prelude::*;
use crate::userdata::{ObjectRef, RBXScriptSignal};

use bevy::prelude::*;
use bevy_rblx_derive::{register, register_class};
use mlua::prelude::*;

#[derive(Resource, Default)]
pub struct RblxLogs {
    pub messages: Vec<(MessageType, String, i64)>,
}

#[derive(Message, Clone)]
pub struct LoggedMessage {
    pub msg_type: MessageType,
    pub msg: String,
    pub time: i64,
}

pub fn push_lua_error(lua: &Lua, error: LuaError) {
    let formatted = match error {
        LuaError::RuntimeError(m) => m,
        e => e.to_string(),
    };
    push_log(lua, MessageType::MessageError, formatted);
}

pub fn push_log(lua: &Lua, msg_type: MessageType, msg: impl std::fmt::Display) {
    let msg = msg.to_string();
    let world_access = WorldAccess::fetch_readonly(lua);
    let mut commands = world_access.access_commands();
    let instant = system_time();
    commands.write_message(LoggedMessage {
        msg_type,
        msg: msg.clone(),
        time: instant,
    });
    {
        let msg_clone = msg.clone();
        commands.queue(move |w: &mut World| -> Result<()> {
            w.resource_mut::<RblxLogs>()
                .messages
                .push((msg_type, msg_clone, instant));
            Ok(())
        });
    }
    match msg_type {
        MessageType::MessageOutput | MessageType::MessageInfo => {
            bevy::log::info!(target: "bevy_rblx::LogService", "{msg}")
        }
        MessageType::MessageWarning => bevy::log::warn!(target: "bevy_rblx::LogService", "{msg}"),
        MessageType::MessageError => bevy::log::error!(target:"bevy_rblx::LogService", "{msg}"),
    }
}

#[register]
impl LuaSingleton for RblxLogs {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.globals().raw_set(
            "print",
            lua.create_function(|lua, mv: LuaMultiValue| -> LuaResult<()> {
                let mut s = String::new();
                let mut p = false;
                for i in mv {
                    if p {
                        s.push('\t');
                    }
                    p = true;
                    s.push_str(i.to_string()?.as_str());
                }
                push_log(lua, MessageType::MessageOutput, s);
                Ok(())
            })?,
        )?;
        lua.globals().raw_set(
            "warn",
            lua.create_function(|lua, mv: LuaMultiValue| -> LuaResult<()> {
                let mut s = String::new();
                let mut p = false;
                for i in mv {
                    if p {
                        s.push('\t');
                    }
                    p = true;
                    s.push_str(i.to_string()?.as_str());
                }
                push_log(lua, MessageType::MessageWarning, s);
                Ok(())
            })?,
        )
    }
}

fn build_message(s: String, t: &LuaTable) -> LuaResult<String> {
    let mut new_string = String::new();
    let mut escaped_string = String::new();
    let mut escape = false;
    for c in s.chars() {
        match (c, escape) {
            ('{', false) => escape = true,
            ('{', true) if escaped_string.is_empty() => {
                escape = false;
                new_string.push('{');
            }
            ('}', true) => {
                new_string += &t
                    .raw_get::<LuaValue>(take(&mut escaped_string))?
                    .to_string()?;
                escape = false;
            }
            (c, false) => new_string.push(c),
            (c, true) => escaped_string.push(c),
        }
    }
    Ok(new_string)
}

const LOG_SERVICE_ERROR_MT: &'static str = "LOG_SERVICE_ERROR_METATABLE";

register_class! {
    #[post_init=fn(lua: &Lua, _this: Entity) -> LuaResult<()> {
        let table = lua.create_table()?;
        table.raw_set("__tostring", lua.create_function(move |_lua: &Lua, this: LuaTable| this.raw_get::<LuaString>("message"))?)?;
        table.set_readonly(true);
        lua.set_named_registry_value(LOG_SERVICE_ERROR_MT, table)?;
        Ok(())
    }]
    #[require_components(DisablingService)]
    priv LogService(Service)
    members {
        #[read_only]
        message_out: RBXScriptSignal
    }
    methods {
        fn clear_output(lua: &Lua, this: ObjectRef) -> LuaResult<()> {
            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;

            world.resource_mut::<RblxLogs>().messages.clear();
            bevy::log::info!(target: "bevy_rblx::LogService", "OUTPUT CLEAR REQUESTED");
            Ok(())
        }
        fn error(lua: &Lua, this: ObjectRef, message: String, context: Option<LuaTable>) -> LuaResult<()> {
            let msg;
            let tbl = lua.create_table()?;
            tbl.raw_set("template", message.clone())?;
            tbl.raw_set("stack", lua.traceback(None, 2)?)?;
            if let Some(ctx) = context {
                msg = build_message(message, &ctx)?;
                tbl.raw_set("context", ctx)?;
            } else {
                msg = message;
            }
            push_log(lua, MessageType::MessageError, &msg);
            tbl.raw_set("message", msg.clone())?;
            Err(LuaError::runtime(msg))
        }
        fn info(lua: &Lua, this: ObjectRef, message: String, context: Option<LuaTable>) -> LuaResult<()> {
            if let Some(ctx) = context {
                push_log(lua, MessageType::MessageInfo, build_message(message, &ctx)?);
            } else {
                push_log(lua, MessageType::MessageInfo, message);
            }
            Ok(())
        }
        fn warn(lua: &Lua, this: ObjectRef, message: String, context: Option<LuaTable>) -> LuaResult<()> {
            if let Some(ctx) = context {
                push_log(lua, MessageType::MessageWarning, build_message(message, &ctx)?);
            } else {
                push_log(lua, MessageType::MessageWarning, message);
            }
            Ok(())
        }
        fn output(lua: &Lua, this: ObjectRef, message: String, context: Option<LuaTable>) -> LuaResult<()> {
            if let Some(ctx) = context {
                push_log(lua, MessageType::MessageOutput, build_message(message, &ctx)?);
            } else {
                push_log(lua, MessageType::MessageOutput, message);
            }
            Ok(())
        }
        fn log(lua: &Lua, this: ObjectRef, ty: MessageType, message: String, context: Option<LuaTable>) -> LuaResult<()> {
            if let Some(ctx) = context {
                push_log(lua, ty, build_message(message, &ctx)?);
            } else {
                push_log(lua, ty, message);
            }
            Ok(())
        }
        fn get_log_history(lua: &Lua, this: ObjectRef) -> LuaResult<LuaTable> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            let t = lua.create_table()?;
            let mut i = 1;
            for (ty, msg, time) in world.resource::<RblxLogs>().messages.iter() {
                let entry = lua.create_table()?;
                entry.raw_set("type", *ty)?;
                entry.raw_set("message", msg.clone())?;
                entry.raw_set("timestamp", *time)?;
                t.raw_set(i, entry)?;
                i += 1;
            }
            Ok(t)
        }
    }
}
