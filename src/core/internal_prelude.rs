pub(crate) use crate as bevy_rblx;
pub(crate) use crate::core::refcounted::RefCountedEntityCommandsExt as _;

mod sealed {
    use std::any::type_name;

    use mlua::prelude::*;

    pub(crate) trait IntoLuaThread {
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
}

pub(crate) use sealed::AnyUserDataTypedExt as _;
pub(crate) use sealed::IntoLuaThread;
