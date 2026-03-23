use bevy_rblx_derive::lua_enum;

#[lua_enum]
pub enum RunContext {
    Legacy,
    Server,
    Client,
    Plugin,
}

impl Default for RunContext {
    fn default() -> Self {
        Self::Legacy
    }
}