use bevy_rblx_derive::lua_enum;

#[lua_enum(default=Immediate)]
pub enum SignalBehavior {
    Default,
    Immediate,
    Deferred,
    AncestryDeferred
}