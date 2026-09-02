use std::sync::Arc;

use crate::internal_prelude::*;
use bevy::prelude::*;
use mlua::prelude::*;

use crate::core::object::{InstanceMembers, ServiceMembers};
use crate::core::{Headless, WorldAccess};
use crate::enums::{
    KeyCode, KeyCodeStringFormat, ModifierKey, MouseBehavior, PreferredInput, UserInputState,
    UserInputType,
};
use crate::userdata::{CFrame, ObjectRef, RBXScriptSignal, Vector2, Vector3};
use bevy_rblx_derive::register_class;

register_class! {
    #[post_init=fn(lua: &Lua, this: Entity) -> LuaResult<()> {
        let mut wa = WorldAccess::fetch(lua);
        let world = wa.access_synchronized()?;
        let is_server = world.contains_resource::<Headless>();

        if !is_server {
            let mut c = wa.access_commands();
            c.entity(this).insert(UserInputServiceMembers {
                keyboard_enabled: true,
                mouse_enabled: true,
                ..default()
            });
        }
        Ok(())
    }]
    priv UserInputService(Service)
    members {
        #[read_only]
        accelerometer_enabled: bool,
        #[read_only]
        gamepad_enabled: bool,
        #[read_only]
        gyroscope_enabled: bool,
        #[read_only]
        keyboard_enabled: bool,
        #[default=1.0]
        pub mouse_delta_sensitivity: f64,
        #[read_only]
        mouse_enabled: bool,
        #[default=MouseBehavior::Default]
        pub mouse_behavior: MouseBehavior,
        #[setter=fn(lua: &Lua, this: Entity, ctx: &mut ObjectContext, v: LuaValue) -> LuaResult<()> {
            lua_todo!()
        }]
        mouse_icon: String,
        #[default=true]
        #[setter=fn(lua: &Lua, this: Entity, ctx: &mut ObjectContext, v: LuaValue) -> LuaResult<()> {
            lua_todo!()
        }]
        pub mouse_icon_enabled: bool,
        #[read_only]
        on_screen_keyboard_position: Vector2,
        #[read_only]
        on_screen_keyboard_size: Vector2,
        #[read_only]
        on_screen_keyboard_visible: bool,
        #[read_only]
        #[default=PreferredInput::KeyboardAndMouse]
        preferred_input: PreferredInput,
        #[read_only]
        touch_enabled: bool,
        #[read_only]
        touch_screen_enabled: bool,

        priv keys_pressed: Arc<[Entity]>,
        priv modifier_keys_pressed: Vec<ModifierKey>,

        #[read_only]
        pub device_acceleration_changed: RBXScriptSignal,
        #[read_only]
        pub device_gravity_changed: RBXScriptSignal,
        #[read_only]
        pub device_rotation_changed: RBXScriptSignal,
        #[read_only]
        #[rename="GamepadConnected"]
        pub gamepad_connected_event: RBXScriptSignal,
        #[read_only]
        pub gamepad_disconnected: RBXScriptSignal,
        #[read_only]
        pub input_began: RBXScriptSignal,
        #[read_only]
        pub input_changed: RBXScriptSignal,
        #[read_only]
        pub input_ended: RBXScriptSignal,
        #[read_only]
        pub jump_request: RBXScriptSignal,
        #[read_only]
        pub last_input_type_changed: RBXScriptSignal,
        #[read_only]
        pub pointer_action: RBXScriptSignal,
        #[read_only]
        pub text_box_focused: RBXScriptSignal,
        #[read_only]
        pub text_box_focus_released: RBXScriptSignal,
        #[read_only]
        pub touch_drag: RBXScriptSignal,
        #[read_only]
        pub touch_ended: RBXScriptSignal,
        #[read_only]
        pub touch_long_press: RBXScriptSignal,
        #[read_only]
        pub touch_moved: RBXScriptSignal,
        #[read_only]
        pub touch_pan: RBXScriptSignal,
        #[read_only]
        pub touch_pinch: RBXScriptSignal,
        #[read_only]
        pub touch_rotate: RBXScriptSignal,
        #[read_only]
        pub touch_started: RBXScriptSignal,
        #[read_only]
        pub touch_swipe: RBXScriptSignal,
        #[read_only]
        pub touch_tap: RBXScriptSignal,
        #[read_only]
        pub touch_tap_in_world: RBXScriptSignal,
        #[read_only]
        pub window_focused: RBXScriptSignal,
        #[read_only]
        pub window_focus_released: RBXScriptSignal

    }
    methods {
        fn create_virtual_input(lua: &Lua, this: ObjectRef) -> LuaResult<ObjectRef> {
            lua_todo!()
        }
        fn gamepad_supports(lua: &Lua, this: ObjectRef, gamepad_num: UserInputType, gamepadKeyCode: KeyCode) -> LuaResult<bool> {
            lua_todo!()
        }
        fn get_connected_gamepads(lua: &Lua, this: ObjectRef) -> LuaResult<Vec<UserInputType>> {
            lua_todo!()
        }
        fn get_device_acceleration(lua: &Lua, this: ObjectRef) -> LuaResult<ObjectRef> {
            lua_todo!()
        }
        fn get_device_gravity(lua: &Lua, this: ObjectRef) -> LuaResult<ObjectRef> {
            lua_todo!()
        }
        fn get_device_rotation(lua: &Lua, this: ObjectRef) -> LuaResult<(ObjectRef, CFrame)> {
            lua_todo!()
        }
        fn get_focused_textbox(lua: &Lua, this: ObjectRef) -> LuaResult<Option<ObjectRef>> {
            lua_todo!()
        }
        fn get_gamepad_connected(lua: &Lua, this: ObjectRef, gamepad_num: UserInputType) -> LuaResult<bool> {
            lua_todo!()
        }
        fn get_gamepad_state(lua: &Lua, this: ObjectRef, gamepad_num: UserInputType) -> LuaResult<Vec<ObjectRef>> {
            lua_todo!()
        }
        fn get_image_for_keycode(lua: &Lua, this: ObjectRef, key_code: KeyCode) -> LuaResult<String> {
            lua_todo!()
        }
        fn get_last_input_type(lua: &Lua, this: ObjectRef) -> LuaResult<UserInputType> {
            lua_todo!()
        }
        fn get_mouse_buttons_pressed(lua: &Lua, this: ObjectRef) -> LuaResult<Vec<ObjectRef>> {
            lua_todo!()
        }
        fn get_mouse_delta(lua: &Lua, this: ObjectRef) -> LuaResult<Vector2> {
            lua_todo!()
        }
        fn get_mouse_location(lua: &Lua, this: ObjectRef) -> LuaResult<Vector2> {
            lua_todo!()
        }
        fn get_navigation_gamepads(lua: &Lua, this: ObjectRef) -> LuaResult<Vec<ObjectRef>> {
            lua_todo!()
        }
        fn get_string_for_keycode(lua: &Lua, this: ObjectRef, key_code: KeyCode, format: Option<KeyCodeStringFormat>) -> LuaResult<String> {
            // literally tostring it
            let s = key_code.into_lua(lua)?.to_string()?;
            if format.unwrap_or_default() == KeyCodeStringFormat::Abbreviated {
                Ok(match key_code {
                    KeyCode::BackSpace => "Bksp",
                    KeyCode::Clear => "Clr",
                    KeyCode::Escape => "Esc",
                    KeyCode::QuotedDouble => "\"",
                    KeyCode::Hash => "#",
                    KeyCode::Dollar => "$",
                    KeyCode::Percent => "%",
          KeyCode::Ampersand => "&",
                    KeyCode::Quote => "'",
                    KeyCode::LeftParenthesis => "(",
                    KeyCode::RightParenthesis => ")",
                    KeyCode::Asterisk => "*",
                    KeyCode::Plus => "+",
                    KeyCode::Comma => ",",
                    KeyCode::Minus => "-",
                    KeyCode::Period => ".",
                    KeyCode::Slash => "/",
                    KeyCode::Zero => "0",
                    KeyCode::One => "1",
                    KeyCode::Two => "2",
                    KeyCode::Three => "3",
                    KeyCode::Four => "4",
                    KeyCode::Five => "5",
                    KeyCode::Six => "6",
                    KeyCode::Seven => "7",
                    KeyCode::Eight => "8",
                    KeyCode::Nine => "9",
                    KeyCode::Colon => ",",
                    KeyCode::Semicolon => ";",
                    KeyCode::LessThan => "<",
                    KeyCode::Equals => "=",
                    KeyCode::GreaterThan => ">",
                    KeyCode::Question => "?",
                    KeyCode::At => "@",
                    KeyCode::LeftBracket => "[",
                    KeyCode::BackSlash => "\\",
                    KeyCode::RightBracket => "]",
                    KeyCode::Caret => "^",
                    KeyCode::Underscore => "_",
                    KeyCode::Backquote => "`",
                    KeyCode::LeftCurly => "{",
                    KeyCode::Pipe => "|",
                    KeyCode::RightCurly => "}",
                    KeyCode::Tilde => "~",
                    KeyCode::Delete => "Del",
                    KeyCode::KeypadZero => "Num0",
                    KeyCode::KeypadOne => "Num1",
                    KeyCode::KeypadTwo => "Num2",
                    KeyCode::KeypadThree => "Num3",
                    KeyCode::KeypadFour => "Num4",
                    KeyCode::KeypadFive => "Num5",
                    KeyCode::KeypadSix => "Num6",
                    KeyCode::KeypadSeven => "Num7",
                    KeyCode::KeypadEight => "Num8",
                    KeyCode::KeypadNine => "Num9",
                    KeyCode::KeypadEnter => "NumEnter",
                    KeyCode::KeypadEquals => "NumEqual",
                    KeyCode::LeftShift => "LShift",
                    KeyCode::RightControl => "RCtrl",
                    KeyCode::LeftControl => "LCtrl",
                    KeyCode::RightAlt => "RAlt",
                    KeyCode::LeftAlt => "LAlt",
                    KeyCode::RightMeta => "RMeta",
                    KeyCode::LeftMeta => "LMeta",
                    KeyCode::RightSuper => "RSuper",
                    KeyCode::LeftSuper => "LSuper",
                    KeyCode::ButtonX => "X",
                    KeyCode::ButtonY => "Y",
                    KeyCode::ButtonA => "A",
                    KeyCode::ButtonB => "B",
                    KeyCode::ButtonR1 => "R1",
                    KeyCode::ButtonL1 => "L1",
                    KeyCode::ButtonR2 => "R2",
                    KeyCode::ButtonL2 => "L2",
                    KeyCode::ButtonR3 => "R3",
                    KeyCode::ButtonL3 => "L3",
                    KeyCode::MouseLeftButton => "LMB",
                    KeyCode::MouseMiddleButton => "MMB",
                    KeyCode::MouseRightButton => "RMB",
                    _ => &s[const {"KeyCode.".len()}..]
                }.to_string())
            } else {
                let name = &s[const {"KeyCode.".len()}..];
                Ok(name.to_string())
            }
        }
    }
}

register_class! {
    #[post_init=fn(lua: &Lua, this: Entity) -> LuaResult<()> {
        let mut wa = WorldAccess::fetch(lua);
        let world = wa.access_synchronized()?;
        let mut m = InstanceMembers::fetch_members_mut(world, this);
        m.parent_protected = true;
        m.cloning_protected = true;
        m.destroy_protected = true;
        Ok(())
    }]
    priv InputObject(Instance)
    members {
        #[read_only]
        pub delta: Vector3,
        #[read_only]
        #[default=KeyCode::None]
        pub keycode: KeyCode,
        #[read_only]
        pub position: Vector3,
        #[read_only]
        #[default=UserInputState::Begin]
        pub user_input_state: UserInputState,
        #[read_only]
        #[default=UserInputType::None]
        pub user_input_type: UserInputType,
        pub priv modifier_keys: Vec<ModifierKey>
    }
    methods {
        fn is_modifier_key_down(lua: &Lua, this: ObjectRef, key: ModifierKey) -> LuaResult<bool> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            Ok(InputObjectMembers::fetch_members(&world, this.entity()).modifier_keys.contains(&key))
        }
    }
}
