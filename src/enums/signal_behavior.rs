use bevy_rblx_derive::lua_enum;

#[lua_enum(default=Immediate)]
pub enum SignalBehavior {
    Default,
    Immediate,
    Deferred,
    AncestryDeferred
}

impl From<u64> for SignalBehavior {
    fn from(value: u64) -> Self {
        match value {
            1 => SignalBehavior::Immediate,
            2 => SignalBehavior::Deferred,
            3 => SignalBehavior::AncestryDeferred,
            _ => SignalBehavior::Default
        }
    }
}