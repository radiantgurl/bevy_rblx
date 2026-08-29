use bevy_rblx_derive::lua_enum;

#[lua_enum]
pub enum ModifierKey {
    Shift,
    Ctrl,
    Alt,
    Meta,
}
