use std::io::Read;

use bevy::prelude::*;
use bevy_egui::{
    EguiContexts, EguiPrimaryContextPass,
    egui::{
        self, Color32, FontId, ScrollArea, TextEdit, TextFormat, Window,
        text::{CCursor, CCursorRange, LayoutJob},
        text_edit::TextEditState,
    },
};
use chrono::DateTime;
use mlua::prelude::*;

use crate::{
    core::{
        LoggedMessage, LuauContainer, RblxLogs, TaskScheduler, ThreadIdentity, ThreadIdentityType,
        WorldAccess, instance::RootInstance, push_log, push_lua_error,
    },
    enums::MessageType,
    instance::WorkspaceMembers,
    userdata::ObjectRef,
};

async fn interpreter_execute(lua: Lua, (e, table, chunk_name): (String, LuaTable, String)) -> LuaResult<()> {
    println!("{e}");
    let res = lua
        .load(e)
        .set_environment(table.clone())
        .set_name(chunk_name)
        .eval_async::<LuaMultiValue>()
        .await;
    match res {
        Ok(v) => {
            if !v.is_empty() {
                let s = v
                    .into_iter()
                    .map(|x| {
                        x.to_string()
                            .unwrap_or_else(|_| format!("<error occured while running tostring>"))
                    })
                    .reduce(|a, b| format!("{a}\t{b}"))
                    .unwrap();
                push_log(&lua, crate::enums::MessageType::MessageOutput, s);
            }
        }
        Err(e) => {
            push_lua_error(&lua, e);
        }
    }
    Ok(())
}

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
    let env = lua.create_table()?;
    lua.globals()
        .for_each(|k: LuaValue, v: LuaValue| env.raw_set(k, v))
        .unwrap();
    let env_copy = lua.create_table()?;
    let env_copy_mt = lua.create_table()?;
    env_copy_mt.raw_set("__weak","v")?;
    env_copy.set_metatable(Some(env_copy_mt))?;
    env.for_each(|k: LuaValue,v: LuaValue| env_copy.raw_set(k,v))?;
    {
        let table_clone = env.clone();
        let table_clone2 = env_copy.clone();
        TaskScheduler::fetch(&lua).defer(
            &lua,
            lua.create_function(move |lua: &Lua, ()| {
                let game;
                let workspace;
                {
                    let wa = WorldAccess::fetch_readonly(lua);
                    let world = wa.access_read_only();
                    game = world
                        .try_query_filtered::<Entity, With<RootInstance>>()
                        .unwrap()
                        .single(&*world)
                        .unwrap();
                    workspace = world
                        .try_query_filtered::<Entity, With<WorkspaceMembers>>()
                        .unwrap()
                        .single(&*world)
                        .unwrap();
                }
                table_clone.raw_set("game", ObjectRef::new(lua, game))?;
                table_clone.raw_set("workspace", ObjectRef::new(lua, workspace))?;
                table_clone2.raw_set("game", ObjectRef::new(lua, game))?;
                table_clone2.raw_set("workspace", ObjectRef::new(lua, workspace))
            })?,
            (),
        )?;
    }
    env.raw_set("executefile", lua.create_function(move |lua: &Lua, filename: String| {
        let mut file = std::fs::File::open(filename.as_str()).into_lua_err()?;
        let mut data = String::new();
        file.read_to_string(&mut data).into_lua_err()?;
        TaskScheduler::fetch(&lua).defer_custom_pd(&lua, lua.create_async_function(interpreter_execute)?, (data, env_copy.clone(), format!("@{filename}")), false)?;
        Ok(())
    })?)?;
    loop {
        let e = lua.yield_with::<String>(()).await?;

        TaskScheduler::fetch(&lua).defer_custom_pd(
            &lua,
            lua.create_async_function(interpreter_execute)?,
            (e, env.clone(), "=interpreter"),
            false,
        )?;
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

#[derive(Resource)]
pub(super) struct InterpreterThread(LuaThread);
fn ui_commandline(
    mut contexts: EguiContexts,

    mut code_input: Local<String>,
    thread: Res<InterpreterThread>,

    old_logs: Res<RblxLogs>,
    mut new_logs: MessageReader<LoggedMessage>,
    mut current_logs: Local<Option<LayoutJob>>,
    mut clear_logs: Local<bool>,

    mut window_open: Local<bool>,
) -> Result {
    const OUTPUT_COLOR: Color32 = Color32::from_gray(200);
    const INFO_COLOR: Color32 = Color32::from_rgb(0, 111, 196);
    const WARN_COLOR: Color32 = Color32::from_rgb(252, 175, 53);
    const ERROR_COLOR: Color32 = Color32::from_rgb(209, 54, 37);

    if !*clear_logs && old_logs.messages.is_empty() {
        *clear_logs = true;
        *current_logs = None;
    } else {
        *clear_logs = old_logs.messages.is_empty();
    }

    if current_logs.is_none() {
        *current_logs = Some(LayoutJob::default());
        for (msg_type, str, timestamp) in old_logs.messages.iter() {
            let timestamp = DateTime::from_timestamp_secs(*timestamp)
                .map(|x| {
                    x.with_timezone(&chrono::Local)
                        .format("%H:%M:%S")
                        .to_string()
                })
                .unwrap_or_else(|| "--------".into());
            let (color, output_ty) = match msg_type {
                MessageType::MessageOutput => (OUTPUT_COLOR, "  "),
                MessageType::MessageInfo => (INFO_COLOR, "I "),
                MessageType::MessageWarning => (WARN_COLOR, "W "),
                MessageType::MessageError => (ERROR_COLOR, "E "),
            };
            let fmt_str = format!(
                "{}{output_ty}[{timestamp}] {str}",
                if current_logs.as_ref().unwrap().is_empty() {
                    ""
                } else {
                    "\n"
                }
            );
            current_logs.as_mut().unwrap().append(
                &fmt_str,
                0.0,
                TextFormat {
                    color,
                    font_id: FontId::monospace(FontId::default().size),

                    ..default()
                },
            );
        }
        new_logs.clear();
    }

    for LoggedMessage {
        msg_type,
        msg,
        time,
    } in new_logs.read()
    {
        let (color, output_ty) = match msg_type {
            MessageType::MessageOutput => (OUTPUT_COLOR, "  "),
            MessageType::MessageInfo => (INFO_COLOR, "I "),
            MessageType::MessageWarning => (WARN_COLOR, "W "),
            MessageType::MessageError => (ERROR_COLOR, "E "),
        };
        let time = DateTime::from_timestamp_secs(*time)
            .map(|x| {
                x.with_timezone(&chrono::Local)
                    .format("%H:%M:%S")
                    .to_string()
            })
            .unwrap_or_else(|| "--------".into());
        let fmt_str = format!(
            "{}{output_ty}[{time}] {msg}",
            if current_logs.as_ref().unwrap().is_empty() {
                ""
            } else {
                "\n"
            }
        );
        current_logs.as_mut().unwrap().append(
            &fmt_str,
            0.0,
            TextFormat {
                color,
                font_id: FontId::monospace(FontId::default().size),
                ..default()
            },
        );
    }

    if contexts.ctx_mut()?.input(|i| i.key_pressed(egui::Key::F9)) {
        *window_open = !*window_open;
    }
    Window::new("Developer Console")
        .open(&mut window_open)
        .collapsible(false)
        .show(contexts.ctx_mut()?, |ui| {
            let max_x = ui.ctx().viewport_rect().max.x * 0.75;
            ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_width(max_x)
                .max_height(ui.ctx().viewport_rect().max.y * 0.75)
                .auto_shrink(false)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        let label = egui::Label::new(current_logs.as_ref().unwrap().clone())
                            .halign(egui::Align::Min);
                        ui.add(label);
                    })
                });
            let single_line = ui.add_sized(egui::vec2(max_x, FontId::default().size), {
                TextEdit::singleline(&mut *code_input).code_editor()
            });
            if code_input.len() < 2 {
                *code_input = "> ".into();
            }

            ui.ctx().data_mut(|d| {
                if let Some(mut state) = d.get_persisted::<TextEditState>(single_line.id) {
                    if let Some(r) = state.cursor.char_range()
                        && (r.contains(CCursorRange::one(CCursor::new(0)))
                            || r.contains(CCursorRange::one(CCursor::new(1))))
                    {
                        let mut cc = r.sorted_cursors();
                        cc[0].index = 2;
                        state
                            .cursor
                            .set_char_range(Some(CCursorRange::two(cc[0], cc[1])));
                        d.insert_persisted(single_line.id, state);
                    }
                }
            });
            if single_line.lost_focus()
                && single_line.ctx.input(|i| i.key_pressed(egui::Key::Enter))
            {
                thread.0.resume::<()>(code_input[2..].to_string()).unwrap();
                let is_empty = if current_logs.as_ref().unwrap().is_empty() {
                    ""
                } else {
                    "\n"
                };
                current_logs.as_mut().unwrap().append(
                    &format!("{is_empty}{}", code_input.as_str()),
                    0.0,
                    TextFormat {
                        color: Color32::from_gray(127),
                        font_id: FontId::monospace(FontId::default().size),
                        ..default()
                    },
                );
                *code_input = "> ".into();
                single_line.request_focus();
            }
            if single_line.ctx.input(|i| i.key_pressed(egui::Key::F9)) {
                single_line.request_focus();
            }
        });
    Ok(())
}
