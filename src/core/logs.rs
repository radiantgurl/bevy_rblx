use std::time::Instant;

use crate::core::{Instance, LuaSingleton, ThreadIdentity, WorldAccess};
use crate::enums::MessageType;
use crate::userdata::ObjectRef;
use crate::internal_prelude::*;

use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::prelude::*;

#[derive(Resource, Default)]
pub struct RblxLogs {
    pub messages: Vec<(MessageType, String, Instant)>,
}

#[derive(Message, Clone)]
pub struct LoggedMessage {
    msg_type: MessageType,
    msg: String,
    time: Instant,
}

pub fn push_lua_error(lua: &Lua, thread: LuaThread, error: LuaError) {
    let path = {
        let ti = ThreadIdentity::get_thread(lua, &thread);
        if let Some(e) = ti.script {
            Instance::get_full_name(lua, (ObjectRef::new(lua, e),)).unwrap()
        } else {
            String::from("anonymous")
        }
    };
    push_log(lua, MessageType::MessageError, format!("[{path}] {error}"));
}

pub fn push_log(lua: &Lua, msg_type: MessageType, msg: impl std::fmt::Display) {
    let msg = msg.to_string();
    let world_access = WorldAccess::fetch_readonly(lua);
    let mut commands = world_access.access_commands();
    let instant = Instant::now();
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
            bevy::log::info!(target: "bevy_rblx::logs", "{msg}")
        }
        MessageType::MessageWarning => bevy::log::warn!(target: "bevy_rblx::logs", "{msg}"),
        MessageType::MessageError => bevy::log::error!(target:"bevy_rblx::logs", "{msg}"),
    }
}

#[register]
impl LuaSingleton for RblxLogs {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.globals().raw_set("print", lua.create_function(|lua, mv: LuaMultiValue| -> LuaResult<()> {
            let mut s = String::new();
            let mut p = false;
            for i in mv {
                if p {
                    s.push('\t');
                }
                p = true;
                s.push_str(i.to_string()?.as_str());
            }
            push_log(lua, MessageType::MessageInfo, s);
            Ok(())
        })?)?;
        lua.globals().raw_set("warn", lua.create_function(|lua, mv: LuaMultiValue| -> LuaResult<()> {
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
        })?)
    }
}