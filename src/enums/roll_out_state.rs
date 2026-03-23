use bevy_rblx_derive::lua_enum;

#[lua_enum(default=Default)]
pub enum RollOutState {
    Default,
    Disabled,
    Enabled
}