use bevy_rblx_derive::lua_enum;

#[lua_enum]
pub enum ContentSourceType {
    None,
    Uri,
    Object,
    Opaque
}
