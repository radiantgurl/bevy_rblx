use bevy_rblx_derive::lua_enum;

#[lua_enum]
pub enum StepFrequency {
    Hz60,
    Hz30,
    Hz15,
    Hz10,
    Hz5,
    Hz1
}