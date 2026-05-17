use bevy::math::EulerRot;
use bevy_rblx_derive::lua_enum;

#[lua_enum]
pub enum RotationOrder {
    XYZ,
    XZY,
    YZX,
    YXZ,
    ZXY,
    ZYX,
}

impl Into<EulerRot> for RotationOrder {
    fn into(self) -> EulerRot {
        match self {
            RotationOrder::XYZ => EulerRot::XYZ,
            RotationOrder::XZY => EulerRot::XZY,
            RotationOrder::YZX => EulerRot::YZX,
            RotationOrder::YXZ => EulerRot::YXZ,
            RotationOrder::ZXY => EulerRot::ZXY,
            RotationOrder::ZYX => EulerRot::ZYX,
        }
    }
}