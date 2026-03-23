use bevy_rblx_derive::lua_enum;

#[lua_enum(default=Disabled)]
pub enum PrimalPhysicsSolver {
    Default,
    Experimental,
    Disabled
}