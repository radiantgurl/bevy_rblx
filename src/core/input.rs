use bevy::{prelude::*, tasks::AsyncComputeTaskPool};
use bevy_async_commands::prelude::*;
use mlua::prelude::*;

use crate::{
    core::{
        LuauContainer, TaskScheduler, ThreadIdentity, ThreadIdentityType, WorldAccess,
        instance::RootInstance, push_log, push_lua_error,
    },
    userdata::ObjectRef,
};

pub async fn interpreter(lua: Lua, (): ()) -> LuaResult<()> {
    unsafe {
        ThreadIdentity::set_thread(
            &lua,
            lua.current_thread(),
            ThreadIdentity {
                identity: ThreadIdentityType::StudioCommandBar,
                script: None,
            },
        )
    };

    let table = lua.create_table()?;
    {
        let table_clone = table.clone();
        TaskScheduler::fetch(&lua).defer(
            &lua,
            lua.create_function(move |lua: &Lua, ()| {
                let e;
                {
                    let wa = WorldAccess::fetch_readonly(lua);
                    let world = wa.access_read_only();
                    e = world
                        .try_query_filtered::<Entity, With<RootInstance>>()
                        .unwrap()
                        .single(&*world)
                        .unwrap();
                }
                table_clone.raw_set("game", ObjectRef::new(lua, e))
            })?,
            (),
        )?;
    }
    loop {
        let e = lua.yield_with::<String>(()).await?;

        {
            let thr = lua.current_thread();
            TaskScheduler::fetch(&lua).defer_high_priority(&lua, thr, ())?;
        }
        lua.yield_with::<()>(()).await?; // await World Access

        let res = lua
            .load(e)
            .set_environment(table.clone())
            .set_name("=interpreter")
            .eval_async::<LuaMultiValue>()
            .await;
        match res {
            Ok(v) => {
                if !v.is_empty() {
                    let s = v
                        .into_iter()
                        .map(|x| {
                            x.to_string().unwrap_or_else(|_| {
                                format!("<error occured while running tostring>")
                            })
                        })
                        .reduce(|a, b| format!("{a}\t{b}"))
                        .unwrap();
                    push_log(&lua, crate::enums::MessageType::MessageOutput, s);
                }
            }
            Err(e) => {
                push_lua_error(&lua, e);
            },
        }
    }
}

pub fn start_input_handler(mut commands: Commands) {
    let async_world = commands.async_world();
    AsyncComputeTaskPool::get().spawn(async move {
        let async_world = async_world.await;
        loop {
            let c = dialoguer::Select::new()
                .report(false)
                .item("Luau Console")
                .interact();
            if let Ok(_) = c {
                let mut history = dialoguer::BasicHistory::new();
                let (thr, _lua) = async_world
                    .apply2(|w: &mut World| {
                        let c = w
                            .query_filtered::<&LuauContainer, With<RootInstance>>()
                            .single(w)
                            .unwrap();
                        (
                            c.lua
                                .create_thread(c.lua.create_async_function(interpreter).unwrap())
                                .unwrap(),
                            c.lua.clone()
                        )
                    })
                    .await;
                thr.resume::<()>(()).unwrap();
                loop {
                    let input = dialoguer::Input::<String>::new()
                        .allow_empty(false)
                        .history_with(&mut history)
                        .interact_text();
                    if input.is_err() {
                        println!("Exiting lua shell");
                        break;
                    }
                    thr.resume::<()>(input.unwrap()).unwrap();
                }
            } else {
                async_world.send_message(AppExit::Success).await;
                break;
            }
        }
    }).detach();
}
