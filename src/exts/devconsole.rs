use std::{
    io::{Read, Stdout, stdout},
    mem::{replace, take},
    time::Duration,
};

use crate::{
    core::{
        FAST_FLAGS, Headless, LoggedMessage, LuauContainer, RblxLogs, SchedulerPhase,
        TaskScheduler, ThreadIdentity, WorldAccess,
        extension::{EngineExtension, EngineExtensionDistribution, EngineExtensionInitLevel},
        lua::ThreadIdentityType,
        object::RootInstance,
        push_log, push_lua_error,
    },
    enums::MessageType,
    instance::PlayerMembers,
    internal_prelude::*,
};
use arboard::Clipboard;
use bevy::{
    ecs::{
        schedule::ScheduleCleanupPolicy,
        system::{RunSystemOnce, SystemParam},
    },
    prelude::*,
};
use bevy_egui::{
    EguiContexts, EguiPrimaryContextPass,
    egui::{
        self, Color32, FontId, Response, ScrollArea, TextEdit, TextFormat, Window,
        text::{CCursor, CCursorRange, LayoutJob},
        text_edit::TextEditState,
    },
};
use bevy_rblx_derive::{cached_lua_function, fast_flag, register};
use chrono::DateTime;
use crossterm::{
    event::{
        DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableMouseCapture, Event,
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind, poll, read,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        is_raw_mode_enabled,
    },
};
use mlua::prelude::*;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Rect},
    style::{Color, Style, Styled, Stylize},
    text::Text,
    widgets::{BorderType, Borders, Wrap},
};

#[cached_lua_function]
fn interpreter_execute(
    lua: &Lua,
    (e, table, chunk_name): (String, LuaTable, String),
) -> LuaResult<()> {
    let func = if let Ok(f) = lua
        .load(format!("return {e}"))
        .set_environment(table.clone())
        .set_name(chunk_name.as_str())
        .into_function()
    {
        Ok(f)
    } else {
        lua.load(e)
            .set_environment(table.clone())
            .set_name(chunk_name.as_str())
            .into_function()
    }?;
    let res = func.call::<LuaMultiValue>(());

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
    env_copy_mt.raw_set("__weak", "v")?;
    env_copy.set_metatable(Some(env_copy_mt))?;
    env.for_each(|k: LuaValue, v: LuaValue| env_copy.raw_set(k, v))?;
    env.raw_set(
        "executefile",
        lua.create_function(move |lua: &Lua, filename: String| {
            let mut file = std::fs::File::open(filename.as_str()).into_lua_err()?;
            let mut data = String::new();
            file.read_to_string(&mut data).into_lua_err()?;
            TaskScheduler::fetch(&lua).defer_custom_pd(
                &lua,
                lua.create_function(interpreter_execute)?,
                (data, env_copy.clone(), format!("@{filename}")),
                false,
            )?;
            Ok(())
        })?,
    )?;
    loop {
        let e = lua.yield_with::<LuaValue>(()).await?;
        if !e.is_string() {
            continue;
        }
        let e = e.to_string()?;
        if !is_raw_mode_enabled()? {
            println!("> {e}");
        }

        TaskScheduler::fetch(&lua).defer_custom_pd(
            &lua,
            INTERPRETER_EXECUTE.fetch(&lua),
            (e, env.clone(), "=interpreter"),
            false,
        )?;
    }
}

pub fn create_interpreter_thread(w: &mut World) -> Lua {
    let c = w
        .query_filtered::<&LuauContainer, With<RootInstance>>()
        .single(w)
        .unwrap();
    let lua = c.lua.clone();
    let f = lua.create_async_function(interpreter).unwrap();
    let thr = TaskScheduler::fetch(&lua)
        .defer_custom_pd(&lua, f, (), false)
        .unwrap();
    w.insert_resource(InterpreterThread(thr));
    lua
}

fn send_interpreter_thread_handled(
    thread: &InterpreterThread,
    commands: &mut Commands,
    code: String,
) {
    if thread.0.resume::<()>(code.clone()).is_err() {
        bevy::log::warn!(target:"bevy_rblx::devconsole", "Interpreter thread seems to have died. Creating new environment.");
        commands.queue(move |w: &mut World| {
            let lua = create_interpreter_thread(w);
            TaskScheduler::fetch(&lua)
                .defer_custom_pd(
                    &lua,
                    lua.create_function(move |lua: &Lua, c: String| {
                        let thr = {
                            let mut wa = WorldAccess::fetch(&lua);
                            let world = wa.access_synchronized()?;
                            world.resource::<InterpreterThread>().0.clone()
                        };
                        TaskScheduler::fetch(&lua).defer_next_frame(&lua, thr, c)?;
                        Ok(())
                    })
                    .unwrap(),
                    code,
                    false,
                )
                .unwrap();
        })
    }
}

pub fn start_input_handler(w: &mut World) {
    create_interpreter_thread(w);
    if w.contains_resource::<Headless>() {
        w.schedule_scope(PostUpdate, |_, s| {
            s.add_systems(server_commandline.in_set(SchedulerPhase::Input));
        });
        enable_raw_mode().unwrap();
        execute!(
            stdout(),
            DisableFocusChange,
            EnableMouseCapture,
            DisableBracketedPaste,
            EnterAlternateScreen
        )
        .unwrap();
    } else {
        w.schedule_scope(EguiPrimaryContextPass, |_, s| {
            s.add_systems(ui_commandline);
        });
    }
}

#[derive(Default, Clone, Debug)]
pub struct ConsoleHistory {
    history: Vec<String>,
    history_cur_index: Option<usize>,
    history_last_temp: Option<String>,
}

impl ConsoleHistory {
    #[inline]
    pub fn clear(&mut self) {
        self.history.clear();
        self.history_cur_index = None;
        self.history_last_temp = None;
    }
    #[inline]
    pub fn push(&mut self, msg: String) {
        self.history.push(msg);
        self.history_cur_index = None;
        self.history_last_temp = None;
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
    #[inline]
    pub fn can_go_up(&self) -> bool {
        self.history.len() != 0
            && (self.history_cur_index.is_none() || self.history_cur_index.is_some_and(|v| v != 0))
    }
    pub fn up(&mut self, current_buffer: &mut String) {
        debug_assert!(self.can_go_up());
        if self.history_cur_index.is_none() {
            if let Some(history_last_temp) = self.history_last_temp.take() {
                *current_buffer = history_last_temp;
            } else {
                self.history_cur_index = Some(self.history.len() - 1);
                self.history_last_temp = Some(replace(
                    current_buffer,
                    self.history[self.history_cur_index.unwrap()].clone(),
                ));
            }
        } else {
            *self.history_cur_index.as_mut().unwrap() -= 1;
            *current_buffer = self.history[self.history_cur_index.unwrap()].clone();
        }
    }
    #[inline]
    pub fn can_go_down(&self) -> bool {
        self.history_cur_index.is_some()
    }
    pub fn down(&mut self, current_buffer: &mut String) {
        debug_assert!(self.can_go_down());
        if self.history_cur_index.unwrap() + 1 == self.history.len() {
            self.history_cur_index = None;
            *current_buffer = self.history_last_temp.take().unwrap();
        } else {
            *self.history_cur_index.as_mut().unwrap() += 1;
            *current_buffer = self.history[self.history_cur_index.unwrap()].clone();
        }
    }
    #[inline]
    pub fn cancel(&mut self) {
        self.history_cur_index = None;
    }
}

#[derive(Resource)]
struct InterpreterThread(LuaThread);

#[derive(SystemParam)]
struct ConsoleLogs<'w, 's, T: Send + Sync + 'static> {
    old_logs: Res<'w, RblxLogs>,
    new_logs: MessageReader<'w, 's, LoggedMessage>,
    current_logs: Local<'s, Option<T>>,
    clear_logs: Local<'s, bool>,
}

impl<'w, 's, T: Send + Sync + 'static> ConsoleLogs<'w, 's, T> {
    fn fmt_string(msg_type: MessageType, timestamp: i64, str: &str) -> String {
        let timestamp = DateTime::from_timestamp_secs(timestamp)
            .map(|x| {
                x.with_timezone(&chrono::Local)
                    .format("%H:%M:%S")
                    .to_string()
            })
            .unwrap_or_else(|| "--------".into());
        let output_ty = match msg_type {
            MessageType::MessageOutput => "  ",
            MessageType::MessageInfo => "I ",
            MessageType::MessageWarning => "W ",
            MessageType::MessageError => "E ",
        };
        let fmt_str = format!("{output_ty}[{timestamp}] {str}");
        fmt_str
    }
    fn clear_check(&mut self) {
        if !*self.clear_logs && self.old_logs.messages.is_empty() {
            *self.clear_logs = true;
            *self.current_logs = None;
        } else {
            *self.clear_logs = self.old_logs.messages.is_empty();
        }
    }
}

impl<'w, 's> ConsoleLogs<'w, 's, LayoutJob> {
    pub const OUTPUT_COLOR: Color32 = Color32::from_gray(200);
    pub const INFO_COLOR: Color32 = Color32::from_rgb(0, 111, 196);
    pub const WARN_COLOR: Color32 = Color32::from_rgb(252, 175, 53);
    pub const ERROR_COLOR: Color32 = Color32::from_rgb(209, 54, 37);
    pub const COMMANDLINE_COLOR: Color32 = Color32::from_gray(127);

    const fn type_to_color(msg_type: MessageType) -> Color32 {
        match msg_type {
            MessageType::MessageOutput => Self::OUTPUT_COLOR,
            MessageType::MessageInfo => Self::INFO_COLOR,
            MessageType::MessageWarning => Self::WARN_COLOR,
            MessageType::MessageError => Self::ERROR_COLOR,
        }
    }

    fn update(&mut self) -> bool {
        let mut redraw_requested = false;
        self.clear_check();

        if self.current_logs.is_none() {
            redraw_requested = true;
            *self.current_logs = Some(LayoutJob::default());
            for (msg_type, msg, timestamp) in self.old_logs.messages.iter() {
                let color = Self::type_to_color(*msg_type);
                let mut fmt_str = Self::fmt_string(*msg_type, *timestamp, msg.as_str());
                if !self.current_logs.as_ref().unwrap().is_empty() {
                    fmt_str.insert(0, '\n');
                }

                self.current_logs.as_mut().unwrap().append(
                    &fmt_str,
                    0.0,
                    TextFormat {
                        color,
                        font_id: FontId::monospace(14.0),
                        ..default()
                    },
                );
            }
            self.new_logs.clear();
        } else {
            for LoggedMessage {
                msg_type,
                msg,
                timestamp,
            } in self.new_logs.read()
            {
                redraw_requested = true;
                let color = Self::type_to_color(*msg_type);
                let mut fmt_str = Self::fmt_string(*msg_type, *timestamp, msg.as_str());
                if !self.current_logs.as_ref().unwrap().is_empty() {
                    fmt_str.insert(0, '\n');
                }

                self.current_logs.as_mut().unwrap().append(
                    &fmt_str,
                    0.0,
                    TextFormat {
                        color,
                        font_id: FontId::monospace(FontId::default().size),
                        ..default()
                    },
                );
            }
        }
        redraw_requested
    }

    fn push_commandline(&mut self, mut command: String) {
        if !self.current_logs.as_ref().unwrap().is_empty() {
            command.insert(0, '\n');
        };
        self.current_logs.as_mut().unwrap().append(
            &command,
            0.0,
            TextFormat {
                color: Self::COMMANDLINE_COLOR,
                font_id: FontId::monospace(14.0),
                ..default()
            },
        );
    }
}

impl<'w, 's> ConsoleLogs<'w, 's, Text<'static>> {
    pub const OUTPUT_COLOR: Color = Color::Reset;
    pub const INFO_COLOR: Color = Color::LightBlue;
    pub const WARN_COLOR: Color = Color::Yellow;
    pub const ERROR_COLOR: Color = Color::LightRed;
    pub const COMMANDLINE_COLOR: Color = Color::DarkGray;

    pub const OUTPUT_COLOR_RGB: Color = Color::Rgb(200, 200, 200);
    pub const INFO_COLOR_RGB: Color = Color::Rgb(0, 111, 196);
    pub const WARN_COLOR_RGB: Color = Color::Rgb(252, 175, 53);
    pub const ERROR_COLOR_RGB: Color = Color::Rgb(209, 54, 37);
    pub const COMMANDLINE_COLOR_RGB: Color = Color::Rgb(127, 127, 127);

    const fn type_to_color(msg_type: MessageType, rgb: bool) -> Style {
        match (msg_type, rgb) {
            (MessageType::MessageOutput, false) => Style::new().fg(Self::OUTPUT_COLOR),
            (MessageType::MessageInfo, false) => Style::new().fg(Self::INFO_COLOR),
            (MessageType::MessageWarning, false) => Style::new().fg(Self::WARN_COLOR),
            (MessageType::MessageError, false) => Style::new().fg(Self::ERROR_COLOR),

            (MessageType::MessageOutput, true) => Style::new().fg(Self::OUTPUT_COLOR_RGB),
            (MessageType::MessageInfo, true) => Style::new().fg(Self::INFO_COLOR_RGB),
            (MessageType::MessageWarning, true) => Style::new().fg(Self::WARN_COLOR_RGB),
            (MessageType::MessageError, true) => Style::new().fg(Self::ERROR_COLOR_RGB),
        }
    }

    fn push_commandline(&mut self, command: String) {
        let fg_color = if FAST_FLAGS.fetch::<FFDevConsoleUseRGB>() {
            Self::COMMANDLINE_COLOR_RGB
        } else {
            Self::COMMANDLINE_COLOR
        };
        self.current_logs
            .as_mut()
            .unwrap()
            .push_line(command.fg(fg_color));
    }

    fn update(&mut self) -> bool {
        let mut redraw_requested = false;
        let is_rgb = FAST_FLAGS.fetch::<FFDevConsoleUseRGB>();

        self.clear_check();

        if self.current_logs.is_none() {
            redraw_requested = true;
            *self.current_logs = Some(Text::default());
            for (msg_type, msg, timestamp) in self.old_logs.messages.iter() {
                let style = Self::type_to_color(*msg_type, is_rgb);
                let fmt_str = Self::fmt_string(*msg_type, *timestamp, msg.as_str());
                for i in fmt_str.split('\n') {
                    self.current_logs
                        .as_mut()
                        .unwrap()
                        .push_line(i.to_owned().set_style(style));
                }
            }
            self.new_logs.clear();
        } else {
            for LoggedMessage {
                msg_type,
                msg,
                timestamp,
            } in self.new_logs.read()
            {
                redraw_requested = true;
                let style = Self::type_to_color(*msg_type, is_rgb);
                let fmt_str = Self::fmt_string(*msg_type, *timestamp, msg.as_str());
                for i in fmt_str.split('\n') {
                    self.current_logs
                        .as_mut()
                        .unwrap()
                        .push_line(i.to_owned().set_style(style));
                }
            }
        }
        redraw_requested
    }
}

fn ui_commandline(
    mut logs: ConsoleLogs<LayoutJob>,
    mut history: Local<ConsoleHistory>,
    mut buffer: Local<String>,

    thread: Res<InterpreterThread>,

    mut contexts: EguiContexts,
    mut window_open: Local<bool>,
    mut commands: Commands,
) -> Result {
    fn verify_cursor_pos(single_line: Response) {
        single_line.ctx.data_mut(move |d| {
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
    }
    fn set_cursor_pos(single_line: Response, pos: usize) {
        single_line.ctx.data_mut(move |d| {
            if let Some(mut state) = d.get_persisted::<TextEditState>(single_line.id) {
                state
                    .cursor
                    .set_char_range(Some(CCursorRange::one(CCursor::new(pos))));
                d.insert_persisted(single_line.id, state);
            }
        });
    }

    logs.update();

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
                        let label = egui::Label::new(logs.current_logs.as_ref().unwrap().clone())
                            .halign(egui::Align::Min);
                        ui.add(label);
                    })
                });
            let single_line = ui.add_sized(egui::vec2(max_x, 14.0), {
                TextEdit::singleline(&mut *buffer)
                    .code_editor()
                    .lock_focus(true)
            });
            if buffer.len() < 2 {
                *buffer = "> ".into();
            }

            verify_cursor_pos(single_line.clone());
            if single_line.lost_focus()
                && single_line.ctx.input(|i| i.key_pressed(egui::Key::Enter))
            {
                let code = buffer[2..].to_string();
                send_interpreter_thread_handled(&thread, &mut commands, code);

                history.push(buffer.to_string());
                logs.push_commandline(replace(&mut buffer, String::from("> ")));

                single_line.request_focus();
            }
            if single_line.ctx.input(|i| i.key_pressed(egui::Key::F9)) {
                single_line.request_focus();
            }
            if single_line.has_focus() {
                let mut up_pressed = false;
                let mut down_pressed = false;

                single_line.ctx.input(|i| {
                    up_pressed = i.key_pressed(egui::Key::ArrowUp);
                    down_pressed = i.key_pressed(egui::Key::ArrowDown);
                });
                if up_pressed && history.can_go_up() {
                    history.up(&mut buffer);
                    set_cursor_pos(single_line.clone(), buffer.len());
                }
                if down_pressed && history.can_go_down() {
                    history.down(&mut buffer);
                    set_cursor_pos(single_line.clone(), buffer.len());
                }
            }
        });
    Ok(())
}

fn server_commandline(
    mut terminal: Local<Option<Terminal<CrosstermBackend<Stdout>>>>,

    mut logs: ConsoleLogs<Text>,
    mut line_count: Local<usize>,
    mut current_scroll: Local<Option<usize>>,
    mut current_logs_height: Local<u16>,

    mut history: Local<ConsoleHistory>,

    mut clipboard: Local<Option<Clipboard>>,
    mut buffer: Local<String>,
    mut cursor_position: Local<Option<usize>>,
    mut cursor_insert: Local<bool>,

    thread: Res<InterpreterThread>,

    player_count: Query<Entity, With<PlayerMembers>>,

    mut raise_exit: MessageWriter<AppExit>,
    mut commands: Commands,
) -> Result {
    if terminal.is_none() {
        let stdout = stdout();
        let backend = CrosstermBackend::new(stdout);
        *terminal = Some(ratatui::Terminal::new(backend)?);
    }
    if clipboard.is_none() {
        *clipboard = Some(arboard::Clipboard::new()?);
    }

    let mut redraw_requested = logs.update();
    while poll(Duration::new(0, 0))? {
        let ev = read()?;
        match ev {
            Event::Key(key_event) if key_event.kind != KeyEventKind::Release => match key_event {
                // interrupts
                KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    if buffer.is_empty() {
                        logs.push_commandline(format!("Recieved CTRL+C, exiting app..."));
                        raise_exit.write(AppExit::Success);
                    } else {
                        *cursor_insert = false;
                        *cursor_position = None;
                        buffer.clear();
                        history.cancel();
                        terminal.as_mut().unwrap().clear()?;
                        redraw_requested = true;
                    }
                }
                KeyEvent {
                    code: KeyCode::Char('\\'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char('4'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    logs.push_commandline(format!("Recieved CTRL+\\, exiting app..."));
                    raise_exit.write(AppExit::Success);
                }

                //cursor control
                KeyEvent {
                    code: KeyCode::Left,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if cursor_position.is_some_and(|x| x == 0)
                        || (cursor_position.is_none() && buffer.len() == 0)
                    {
                        break;
                    } else {
                        *cursor_position = Some(cursor_position.unwrap_or(buffer.len()) - 1);
                        redraw_requested = true;
                    }
                }
                KeyEvent {
                    code: KeyCode::Right,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if cursor_position.is_none() {
                        break;
                    } else if let Some(p) = *cursor_position {
                        if p + 1 >= buffer.len() {
                            *cursor_position = None;
                        } else {
                            *cursor_position = Some(p + 1);
                        }
                        redraw_requested = true;
                    }
                }
                KeyEvent {
                    code: KeyCode::Home,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if buffer.len() != 0 {
                        *cursor_position = Some(0);
                        redraw_requested = true;
                    }
                }
                KeyEvent {
                    code: KeyCode::End,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if cursor_position.is_some() {
                        *cursor_position = None;
                        redraw_requested = true;
                    }
                }

                KeyEvent {
                    code: KeyCode::Backspace,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if cursor_position.is_some() {
                        if cursor_position.unwrap() == 0 {
                            break; // cancel
                        }
                        buffer.remove(cursor_position.unwrap() - 1);
                        *cursor_position.as_mut().unwrap() -= 1;
                    } else {
                        buffer.pop();
                    }
                    redraw_requested = true;
                }
                KeyEvent {
                    code: KeyCode::Delete,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if let Some(p) = *cursor_position {
                        if buffer.len() != p {
                            buffer.remove(p);
                        }
                        redraw_requested = true;
                    }
                }
                KeyEvent {
                    code: KeyCode::Char('v'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    if let Some(text) = clipboard.as_mut().unwrap().get_text().ok() {
                        if let Some(p) = *cursor_position {
                            buffer.insert_str(p, &text);
                            *cursor_position.as_mut().unwrap() += text.len();
                        } else {
                            buffer.push_str(&text);
                        }
                        redraw_requested = true;
                    }
                }
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers: modifiers @ KeyModifiers::NONE,
                    ..
                }
                | KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers: modifiers @ KeyModifiers::SHIFT,
                    ..
                } => {
                    let c = if modifiers == KeyModifiers::SHIFT {
                        let v = c.to_uppercase().collect::<Vec<_>>();
                        if v.len() > 1 { c } else { v[0] }
                    } else {
                        c
                    };
                    if let Some(p) = *cursor_position {
                        buffer.insert(p, c);
                        *cursor_position.as_mut().unwrap() += 1;
                    } else {
                        buffer.push(c);
                    }
                    redraw_requested = true;
                }

                // console history
                KeyEvent {
                    code: KeyCode::Up,
                    modifiers: KeyModifiers::NONE,
                    ..
                } if history.can_go_up() => {
                    history.up(&mut buffer);
                    *cursor_position = None;
                    redraw_requested = true;
                }
                KeyEvent {
                    code: KeyCode::Down,
                    modifiers: KeyModifiers::NONE,
                    ..
                } if history.can_go_down() => {
                    history.down(&mut buffer);
                    *cursor_position = None;
                    redraw_requested = true;
                }

                // submit
                KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    history.push(buffer.clone());
                    logs.push_commandline(format!("> {}", *buffer));
                    send_interpreter_thread_handled(&thread, &mut commands, take(&mut buffer));
                    *cursor_position = None;
                    *cursor_insert = false;
                    redraw_requested = true;
                }
                // logs scrolling
                KeyEvent {
                    code: KeyCode::Home,
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    *current_scroll = Some(0);
                    redraw_requested = true;
                }
                KeyEvent {
                    code: KeyCode::End,
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    *current_scroll = None;
                    redraw_requested = true;
                }
                KeyEvent {
                    code: KeyCode::PageUp,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    *current_scroll = Some(
                        (current_scroll.unwrap_or(
                            (*line_count as isize - *current_logs_height as isize).max(0) as usize,
                        ) as isize
                            - (*current_logs_height) as isize)
                            .max(0) as usize,
                    );
                    redraw_requested = true;
                }
                KeyEvent {
                    code: KeyCode::PageDown,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    *current_scroll = Some(
                        (current_scroll.unwrap_or(*line_count) + (*current_logs_height as usize))
                            .min(*line_count),
                    );
                    if *current_scroll
                        == Some(
                            (*line_count as isize - *current_logs_height as isize).max(0) as usize,
                        )
                    {
                        *current_scroll = None; // bottom following
                    }
                    redraw_requested = true;
                }
                _ => (),
            },
            Event::Paste(data) => {
                buffer.push_str(&data);
                redraw_requested = true;
            }
            Event::Resize(x, y) => {
                terminal.as_mut().unwrap().resize(Rect::new(0, 0, x, y))?;
                redraw_requested = true
            }

            Event::Mouse(mouse_event) if mouse_event.kind == MouseEventKind::ScrollUp => {
                let height = (*current_logs_height / 3).max(1) as isize;
                *current_scroll = Some(
                    (current_scroll.unwrap_or(
                        (*line_count as isize - *current_logs_height as isize).max(0) as usize,
                    ) as isize
                        - height)
                        .max(0) as usize,
                );
                redraw_requested = true;
            }
            Event::Mouse(mouse_event) if mouse_event.kind == MouseEventKind::ScrollDown => {
                let height = (*current_logs_height / 3).max(1) as usize;
                if current_scroll.is_some() {
                    *current_scroll = Some(
                        (current_scroll.unwrap_or(
                            (*line_count as isize - *current_logs_height as isize).max(0) as usize,
                        ) + height)
                            .min(*line_count),
                    );
                    if current_scroll.unwrap() == *line_count {
                        *current_scroll = None; // bottom following
                    }
                }
                redraw_requested = true;
            }
            _ => (),
        }
    }
    if redraw_requested {
        let term = terminal.as_mut().unwrap();
        // term.clear()?;
        term.draw(|f| {
            let preprompt = "> ".green().bold();
            let mut console_text = Text::default();
            console_text.push_span(preprompt);
            console_text.push_span(buffer.as_str().reset());

            let console_text =
                ratatui::widgets::Paragraph::new(console_text).wrap(Wrap { trim: false });

            let (console_pos_x, console_pos_y) = {
                let text = format!("> {}", buffer.as_str());
                let console_size_x = f.area().width;
                if let Some(p) = *cursor_position {
                    // accurate cursor check
                    let (mut x, mut y) = (2u16, 0u16);
                    for (i, c) in text.chars().enumerate() {
                        if i >= p {
                            break;
                        }
                        match c {
                            '\n' => {
                                y += 1 + (x / console_size_x);
                                x = 0;
                            }
                            _ => x += 1,
                        }
                    }
                    (x % console_size_x, y + (x / console_size_x))
                } else {
                    // cheaper cursor check
                    (
                        text.split("\n")
                            .map(|s| &s[s.len() - s.len() % console_size_x as usize..])
                            .last()
                            .unwrap()
                            .len() as u16,
                        console_text.line_count(f.area().width) as u16 - 1,
                    )
                }
            };

            let ui_layout = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .margin(0)
                .constraints([
                    Constraint::Fill(1),
                    Constraint::Length(console_text.line_count(f.area().width) as u16),
                    Constraint::Length(1),
                ])
                .split(f.area());

            f.set_cursor_position((console_pos_x, console_pos_y + ui_layout[1].y));
            *current_logs_height = ui_layout[0].height;
            let mut logs_render =
                ratatui::widgets::Paragraph::new(logs.current_logs.as_ref().unwrap().clone())
                    .wrap(Wrap { trim: false });
            *line_count = logs_render.line_count(ui_layout[0].width);
            if *line_count > 32000 {
                let lines = &mut logs.current_logs.as_mut().unwrap().lines;
                *lines = lines.split_off(lines.len() - 10000);
                if current_scroll.is_some_and(|v| v < 10000) {
                    *current_scroll = Some(0);
                }
                logs_render =
                    ratatui::widgets::Paragraph::new(logs.current_logs.as_ref().unwrap().clone())
                        .wrap(Wrap { trim: false });
            }
            if current_scroll.is_some_and(|v| {
                (*line_count as isize) - (v as isize) - (ui_layout[0].height as isize) < 0
            }) {
                *current_scroll = None;
            }
            f.render_widget(
                logs_render.scroll((
                    current_scroll.unwrap_or_else(|| {
                        ((*line_count as isize) - (ui_layout[0].height as isize)).max(0) as usize
                    }) as u16,
                    0,
                )),
                ui_layout[0],
            );
            f.render_widget(console_text, ui_layout[1]);
            {
                let block = ratatui::widgets::Block::new()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::new().gray())
                    .border_type(BorderType::Double);
                f.render_widget(block, ui_layout[2]);

                let player_count = player_count.count();

                let status_bar_layout = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Horizontal)
                    .margin(0)
                    .constraints([
                        Constraint::Length(2),
                        Constraint::Length(
                            "0 players".len() as u16
                                + (player_count as f64).log10().max(0f64).floor() as u16,
                        ),
                        Constraint::Fill(1),
                    ])
                    .split(ui_layout[2]);

                let mut line = ratatui::text::Line::default();
                if player_count == 0 {
                    line.push_span("0 players".dark_gray());
                } else if player_count == 1 {
                    line.push_span("1".blue());
                    line.push_span(" player ".gray());
                } else {
                    line.push_span(format!("{player_count}").blue());
                    line.push_span(" players".gray());
                }
                f.buffer_mut().set_line(
                    status_bar_layout[1].x,
                    status_bar_layout[1].y,
                    &line,
                    status_bar_layout[1].width,
                );
            }
        })?;
    }
    Ok(())
}

#[derive(Default)]
pub struct DevConsoleExtension;
#[register]
impl EngineExtension for DevConsoleExtension {
    fn default_enabled(&self) -> bool {
        cfg!(not(test))
    }
    fn id(&self) -> &'static str {
        "devconsole"
    }

    fn init_level(&self) -> EngineExtensionInitLevel {
        EngineExtensionInitLevel::Runtime
    }
    fn distribution(&self) -> EngineExtensionDistribution {
        EngineExtensionDistribution::Both
    }

    fn dyn_clone(&mut self, _app: &mut App) -> Box<dyn EngineExtension> {
        Box::new(Self)
    }

    fn name(&self) -> &'static str {
        "Developer Console"
    }
    fn description(&self) -> Option<&'static str> {
        Some("Adds a developer console on the F9 key")
    }

    fn dynamically_removable(&self) -> bool {
        true
    }

    fn runtime_init(&self, world: &mut World) {
        world.run_system_once(start_input_handler).unwrap();
    }
    fn pre_shutdown_hook(&self, world: &mut World) {
        world.remove_resource::<InterpreterThread>();
        if world.contains_resource::<Headless>() {
            world.schedule_scope(PostUpdate, |w, s| {
                s.remove_systems_in_set(
                    server_commandline,
                    w,
                    ScheduleCleanupPolicy::RemoveSystemsOnly,
                )
                .unwrap();
            });
            disable_raw_mode().unwrap();
            execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture).unwrap();
        } else {
            world.schedule_scope(EguiPrimaryContextPass, |w, s| {
                s.remove_systems_in_set(
                    ui_commandline,
                    w,
                    ScheduleCleanupPolicy::RemoveSystemsOnly,
                )
                .unwrap();
            });
        }
    }
}

fast_flag!(FFDevConsoleUseRGB: bool = true);
