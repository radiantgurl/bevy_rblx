use bevy_rblx_derive::lua_enum;

#[lua_enum]
pub enum CameraType {
    Fixed,
    Attach,
    Watch,
    Track,
    Follow,
    Custom,
    Scriptable,
    Orbital
}
