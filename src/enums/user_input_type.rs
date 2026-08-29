use bevy_rblx_derive::lua_enum;

#[lua_enum]
pub enum UserInputType {
    MouseButton1,
    MouseButton2,
    MouseButton3,
    MouseWheel,
    MouseMovement,
    Touch = 7,
    Keyboard,
    Focus,
    Accelerometer,
    Gyro,
    Gamepad1,
    Gamepad2,
    Gamepad3,
    Gamepad4,
    Gamepad5,
    Gamepad6,
    Gamepad7,
    Gamepad8,
    TextInput,
    InputMethod,
    None,
}
