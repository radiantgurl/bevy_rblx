use bevy_rblx_derive::lua_enum;

#[lua_enum]
pub enum CloseReason {
    Unknown,
    Maintenance,
    DeveloperShutdown,
    DeveloperUpdate,
    ServerEmpty,
    OutOfMemory
}