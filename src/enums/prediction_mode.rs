use bevy_rblx_derive::lua_enum;

#[lua_enum]
pub enum PredictionMode {
    Automatic,
    On,
    Off
}