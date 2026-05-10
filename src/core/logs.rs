use std::time::Instant;

use crate::core::WorldAccess;
use bevy::prelude::*;
use mlua::ffi::{lua_debugtrace, lua_pop, lua_pushstring, lua_tothread};
use mlua::prelude::*;

use crate::enums::MessageType;

#[derive(Resource)]
pub struct RblxLogs {
    messages: Vec<(MessageType, String, Instant)>,
}

#[derive(Message, Clone)]
pub struct LoggedMessage {
    msg_type: MessageType,
    msg: String,
    time: Instant,
}

pub fn push_lua_error(lua: &Lua, thread: LuaThread, error: LuaError) {
    let traceback: String = unsafe {
        lua.exec_raw(thread, move |l| {
            let thr = lua_tothread(l, -1);
            lua_pop(l, 1);
            lua_pushstring(l, lua_debugtrace(thr));
        })
        .unwrap()
    };
    let msg = format!("{error}\n{traceback}");
    push_log(lua, MessageType::MessageError, msg);
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
            bevy::log::info!(target: "rblx", msg)
        }
        MessageType::MessageWarning => bevy::log::warn!(target: "rblx", msg),
        MessageType::MessageError => bevy::log::error!(target:"rblx", msg),
    }
}
