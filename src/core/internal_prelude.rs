pub(crate) use crate as bevy_rblx;
pub(crate) use crate::core::bevy::ref_counted::RefCountedEntityCommandsExt as _;

mod sealed {
    use std::any::type_name;

    use mlua::{ffi::lua_mainthread, lua_State, prelude::*};

    pub trait IntoLuaThread {
        fn into_lua_thread(self, lua: &Lua) -> LuaResult<LuaThread>;
    }

    impl IntoLuaThread for LuaThread {
        fn into_lua_thread(self, _: &Lua) -> LuaResult<LuaThread> {
            Ok(self)
        }
    }

    impl IntoLuaThread for LuaFunction {
        fn into_lua_thread(self, lua: &Lua) -> LuaResult<LuaThread> {
            lua.create_thread(self)
        }
    }

    impl IntoLuaThread for LuaValue {
        fn into_lua_thread(self, lua: &Lua) -> LuaResult<LuaThread> {
            match self {
                Self::Thread(t) => Ok(t),
                Self::Function(t) => lua.create_thread(t),
                x => Err(LuaError::FromLuaConversionError {
                    from: x.type_name(),
                    to: "function|thread".into(),
                    message: None,
                }),
            }
        }
    }

    pub(crate) trait AnyUserDataTypedExt<T: 'static> {
        fn borrow_typed(&self) -> LuaResult<LuaUserDataRef<T>>;
        fn borrow_typed_mut(&self) -> LuaResult<LuaUserDataRefMut<T>>;
        fn borrow_typed_mut_scoped<R>(&self, f: impl FnOnce(&mut T) -> R) -> LuaResult<R>;
        fn take_typed(&self) -> LuaResult<T>;
    }

    impl<T: 'static> AnyUserDataTypedExt<T> for LuaAnyUserData {
        fn borrow_typed(&self) -> LuaResult<LuaUserDataRef<T>> {
            self.borrow().map_err(|_| LuaError::FromLuaConversionError {
                from: "userdata",
                to: type_name::<T>().split("::").last().unwrap().to_owned(),
                message: None,
            })
        }

        fn borrow_typed_mut(&self) -> LuaResult<LuaUserDataRefMut<T>> {
            self.borrow_mut()
                .map_err(|_| LuaError::FromLuaConversionError {
                    from: "userdata",
                    to: type_name::<T>().split("::").last().unwrap().to_owned(),
                    message: None,
                })
        }

        fn borrow_typed_mut_scoped<R>(&self, f: impl FnOnce(&mut T) -> R) -> LuaResult<R> {
            self.borrow_mut_scoped(f)
                .map_err(|_| LuaError::FromLuaConversionError {
                    from: "userdata",
                    to: type_name::<T>().split("::").last().unwrap().to_owned(),
                    message: None,
                })
        }

        fn take_typed(&self) -> LuaResult<T> {
            self.take().map_err(|e| match e {
                LuaError::UserDataTypeMismatch => LuaError::FromLuaConversionError {
                    from: "userdata",
                    to: type_name::<T>().split("::").last().unwrap().to_owned(),
                    message: None,
                },
                e => e,
            })
        }
    }
    impl<T: 'static> AnyUserDataTypedExt<T> for LuaValue {
        fn borrow_typed(&self) -> LuaResult<LuaUserDataRef<T>> {
            self.as_userdata()
                .map(|x| x.borrow_typed())
                .ok_or_else(|| LuaError::FromLuaConversionError {
                    from: "userdata",
                    to: type_name::<T>().split("::").last().unwrap().to_owned(),
                    message: None,
                })
                .flatten()
        }

        fn borrow_typed_mut(&self) -> LuaResult<LuaUserDataRefMut<T>> {
            self.as_userdata()
                .map(|x| x.borrow_typed_mut())
                .ok_or_else(|| LuaError::FromLuaConversionError {
                    from: "userdata",
                    to: type_name::<T>().split("::").last().unwrap().to_owned(),
                    message: None,
                })
                .flatten()
        }

        fn borrow_typed_mut_scoped<R>(&self, f: impl FnOnce(&mut T) -> R) -> LuaResult<R> {
            self.as_userdata()
                .map(|x| x.borrow_typed_mut_scoped(f))
                .ok_or_else(|| LuaError::FromLuaConversionError {
                    from: "userdata",
                    to: type_name::<T>().split("::").last().unwrap().to_owned(),
                    message: None,
                })
                .flatten()
        }

        fn take_typed(&self) -> LuaResult<T> {
            self.as_userdata()
                .map(|x| x.take())
                .ok_or_else(|| LuaError::FromLuaConversionError {
                    from: "userdata",
                    to: type_name::<T>().split("::").last().unwrap().to_owned(),
                    message: None,
                })
                .flatten()
        }
    }

    pub(crate) trait LuaExt: 'static {
        fn to_pointer(&self) -> *mut lua_State;
    }

    impl LuaExt for Lua {
        fn to_pointer(&self) -> *mut lua_State {
            let mut lua_ptr = std::ptr::null_mut();
            unsafe {
                self.exec_raw::<()>((), |l| {
                    lua_ptr = lua_mainthread(l);
                })
                .unwrap();
            }
            debug_assert!(!lua_ptr.is_null());
            lua_ptr
        }
    }

    pub(crate) trait LuaFunctionExt: 'static {
        fn queue_call<'w, 's>(&self, lua: &Lua, args: impl IntoLuaMulti) -> LuaResult<LuaThread>;
    }

    impl LuaFunctionExt for LuaFunction {
        fn queue_call<'w, 's>(&self, lua: &Lua, args: impl IntoLuaMulti) -> LuaResult<LuaThread> {
            lua.app_data_ref::<TaskScheduler>()
                .expect("task scheduler is init")
                .defer_high_priority(lua, self.clone(), args)
        }
    }

    macro_rules! lua_todo {
        () => {
            {
                return mlua::Result::Err(mlua::Error::runtime(format!("not yet implemented")))
            }
        };
        ($($tt: tt),*) => {
            {
                return mlua::Result::Err(mlua::Error::runtime(format!("not yet implemented: {}", format!($($tt),*))))
            }
        }
    }
    #[allow(unused)]
    macro_rules! lua_unimplemented {
        () => {
            {
                return mlua::Result::Err(mlua::Error::runtime(format!("not implemented")))
            }
        };
        ($($tt: tt),*) => {
            {
                return mlua::Result::Err(mlua::Error::runtime(format!("not implemented: {}", format!($($tt),*))))
            }
        }
    }
    use crate::core::TaskScheduler;

    pub(crate) use {lua_todo, lua_unimplemented};
}

pub(crate) use sealed::AnyUserDataTypedExt as _;
pub use sealed::IntoLuaThread;
pub(crate) use sealed::LuaExt as _;
pub(crate) use sealed::LuaFunctionExt as _;
#[allow(unused)]
pub(crate) use sealed::{lua_todo, lua_unimplemented};
