use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui::{self, RichText}};
use mlua::prelude::*;

use crate::{
    core::{
        LoggedMessage, LuauContainer, RblxLogs, TaskScheduler, ThreadIdentity, ThreadIdentityType, WorldAccess, instance::RootInstance, push_log, push_lua_error
    }, enums::MessageType, userdata::ObjectRef
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
    lua.globals().for_each(|k: LuaValue,v: LuaValue| table.raw_set(k,v)).unwrap();
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
            TaskScheduler::fetch(&lua).defer_custom_pd(&lua, thr, (), false)?;
        }
        lua.yield_with::<()>(()).await?; // await World Access
        println!("{e}");
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
    commands.queue(|w: &mut World| {
        let c = w
            .query_filtered::<&LuauContainer, With<RootInstance>>()
            .single(w)
            .unwrap();
        let f = c.lua.create_async_function(interpreter).unwrap();
        let thr = c.lua.create_thread(f).unwrap();
        thr.resume::<()>(()).unwrap();
        w.insert_resource(InterpreterThread(thr));
        w.schedule_scope(EguiPrimaryContextPass, |_, s| {
            s.add_systems(ui_commandline);
        });
    })
}

// fn insert_richtext(rich_text: &mut Option<RichText>, msg_type: MessageType, msg: String, timestamp: f64) {
//     if let Some(x) = rich_text{
//         x.
//     }
// }

#[derive(Resource)]
struct InterpreterThread(LuaThread);
fn ui_commandline(
    mut contexts: EguiContexts,

    mut code_input: Local<String>,
    thread: Res<InterpreterThread>,
    
    old_logs: Res<RblxLogs>,
    mut new_logs: MessageReader<LoggedMessage>,
    mut current_logs: Local<Option<RichText>>
) -> Result {
    // if current_logs.is_none() {
    //     current_logs
    // }

    egui::Window::new("Developer Console").show(contexts.ctx_mut()?, |ui| {
        // ui.label(RichText::)
        let single_line = ui.text_edit_singleline(&mut *code_input);
        if single_line.lost_focus() && single_line.ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            thread.0.resume::<()>(code_input.to_string()).unwrap();
        }
    });
    Ok(())
}