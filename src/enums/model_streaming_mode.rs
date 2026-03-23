use bevy_rblx_derive::lua_enum;

#[lua_enum(default=Atomic)]
pub enum ModelStreamingMode {
    Default,
    Atomic,
    Persistent,
    PersistentPerPlayer,
    Nonatomic,
}