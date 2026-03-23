use bevy_rblx_derive::lua_enum;

#[lua_enum(default=Enabled)]
pub enum PlayerCharacterDestroyBehavior {
    Default,
    Disabled,
    Enabled,
}