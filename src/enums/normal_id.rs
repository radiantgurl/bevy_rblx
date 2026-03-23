use bevy_rblx_derive::lua_enum;

#[lua_enum]
pub enum NormalId {
    Right,
    Top,
    Back,
    Left,
    Bottom,
    Front,
}
