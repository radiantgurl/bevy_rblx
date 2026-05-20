use crate::core::{Instance, LuaSingleton, ServiceMembers, ThreadIdentity, WorldAccess};
use crate::enums::MessageType;
use crate::internal_prelude::*;
use crate::userdata::{ObjectRef, RBXScriptSignal};

use bevy::prelude::*;
use bevy_rblx_derive::{register, register_class};
use mlua::ffi::lua_clock;
use mlua::prelude::*;

#[derive(Resource, Default)]
pub struct RblxLogs {
    pub messages: Vec<(MessageType, String, f64)>,
}

#[derive(Message, Clone)]
pub struct LoggedMessage {
    pub msg_type: MessageType,
    pub msg: String,
    pub time: f64,
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
    let instant = unsafe { lua_clock() };
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
                push_log(lua, MessageType::MessageInfo, s);
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

fn build_message(s: String, t: LuaTable) -> String {
    todo!()
}

register_class! {
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
            lua_todo!()
        }
        fn info(lua: &Lua, this: ObjectRef, message: String, context: Option<LuaTable>) -> LuaResult<()> {
            lua_todo!()
        }
        fn warn(lua: &Lua, this: ObjectRef, message: String, context: Option<LuaTable>) -> LuaResult<()> {
            lua_todo!()
        }
        fn output(lua: &Lua, this: ObjectRef, message: String, context: Option<LuaTable>) -> LuaResult<()> {
            lua_todo!()
        }
        fn log(lua: &Lua, this: ObjectRef, ty: MessageType, message: String, context: Option<LuaTable>) -> LuaResult<()> {
            lua_todo!()
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
