use std::{iter::Peekable, time::Duration};

use crate::{
    core::{
        LuauContainer, TaskScheduler, ThreadIdentity, WorldAccess,
        lua::clock,
        object::{
            DisabledObject, DisablingService, Instance, ServiceMembers,
            data_model::DataModelMembers,
        },
        push_log,
    },
    enums::MessageType,
    exts::scripting::BaseScriptMembers,
    internal_prelude::*,
    userdata::{ObjectRef, RBXScriptSignal},
};
use bevy::{
    ecs::{
        entity::Entity, hierarchy::Children, query::With, resource::Resource, system::Commands,
        world::World,
    },
    platform::collections::HashMap,
};
use bevy_rblx_derive::register_class;
use mlua::prelude::*;

fn calculate_path(
    lua: &Lua,
    source: Option<ObjectRef>,
    line: Option<usize>,
) -> LuaResult<(Option<Entity>, String, usize)> {
    let (real_line, real_source) = lua
        .inspect_stack(1, move |debug| {
            (
                debug.current_line().unwrap_or_default(),
                debug
                    .names()
                    .name
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| String::from("<unknown>")),
            )
        })
        .unwrap();
    let script = source.or_else(|| {
        ThreadIdentity::fetch(lua)
            .script
            .map(|e| ObjectRef::new(lua, e))
    });
    let entity = script.as_ref().map(|e| e.entity());

    let printed_source = script
        .map(|x| Instance::get_full_name(lua, (x,)))
        .transpose()?
        .unwrap_or(real_source);
    let printed_line = line.unwrap_or(real_line);
    Ok((
        entity,
        format!("{printed_source}:{printed_line}"),
        real_line,
    ))
}

#[derive(Resource, Default)]
pub struct TestServiceLogs {
    pub logs: HashMap<Option<Entity>, Vec<(f64, MessageType, String, String)>>,
    pub warn_count: u64,
    pub error_count: u64,
}

impl TestServiceLogs {
    pub fn push(&mut self, script: Option<Entity>, ty: MessageType, path: String, msg: String) {
        match ty {
            MessageType::MessageWarning => self.warn_count += 1,
            MessageType::MessageError => self.error_count += 1,
            _ => (),
        }
        self.logs
            .entry(script)
            .or_default()
            .push((clock().as_secs_f64(), ty, path, msg))
    }
    pub fn fetch_failed_tests(&self) -> Vec<Option<Entity>> {
        let mut failed_tests = Vec::new();
        for (k, v) in self.logs.iter() {
            'next_test: for (_, ty, _, msg) in v.iter() {
                if *ty == MessageType::MessageError {
                    if msg.contains("Test ended:") {
                        failed_tests.push(*k);
                        break 'next_test;
                    }
                }
            }
        }
        failed_tests
    }
    pub fn print_logs(self, lua: &Lua, script: Option<Entity>) -> Self {
        if let Some(logs) = self.logs.get(&script) {
            for (clock, ty, path, msg) in logs.iter() {
                push_log(lua, *ty, format!("{clock:.3}: [{path}] {msg}"));
            }
        }
        self
    }
}

pub trait TestServiceCommandsExt {
    fn push_entry(&mut self, script: Option<Entity>, ty: MessageType, path: String, msg: String);
}

impl<'w: 's, 's> TestServiceCommandsExt for Commands<'w, 's> {
    fn push_entry(&mut self, script: Option<Entity>, ty: MessageType, path: String, msg: String) {
        self.queue(move |w: &mut World| {
            w.resource_mut::<TestServiceLogs>()
                .push(script, ty, path, msg);
        })
    }
}

register_class! {
    #[require_components(DisablingService)]
    #[post_init=fn(lua: &Lua, _this: Entity) -> LuaResult<()> {
        let wa = WorldAccess::fetch_readonly(lua);
        let mut c = wa.access_commands();
        c.init_resource::<TestServiceLogs>();
        Ok(())
    }]
    priv TestService(Service)
    members {
        priv running: bool,

        #[getter=fn(lua: &Lua, _this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            world.resource::<TestServiceLogs>().warn_count.into_lua(lua)
        }]
        virtual warn_count: u64,

        #[getter=fn(lua: &Lua, _this: Entity, _vtable: &'static ObjectVTable) -> LuaResult<LuaValue> {
            let wa = WorldAccess::fetch_readonly(lua);
            let world = wa.access_read_only();
            world.resource::<TestServiceLogs>().warn_count.into_lua(lua)
        }]
        virtual error_count: u64,

        #[read_only]
        pub test_count: u64,

        #[read_only]
        pub server_collect_conditional_result: RBXScriptSignal,
        #[read_only]
        pub server_collect_result: RBXScriptSignal,

        priv done: RBXScriptSignal,

        pub auto_runs: bool,
        #[default=60.0]
        #[setter=fn(lua: &Lua, this: Entity, ctx: &mut ObjectContext, v: LuaValue) -> LuaResult<()> {
            let value = f64::from_lua(v, lua)?;
            if value.is_infinite() || value.is_nan() || value < 0.0 {
                return Err(LuaError::runtime("invalid number provided for TestService.Timeout: must be finite and above 0"));
            }
            let mut wa = WorldAccess::fetch(lua);
            let world = wa.access_synchronized()?;
            let mut members = TestServiceMembers::fetch_members_mut(world, this);
            if members.timeout != value {
                members.timeout = value;
                ctx.set_changed();
            }
            Ok(())
        }]
        pub timeout: f64
    }
    methods {
        fn check(lua: &Lua, this: ObjectRef, condition: bool, description: String, source: Option<ObjectRef>, line: Option<usize>) -> LuaResult<()> {
            let (script, path, real_line) = calculate_path(lua, source, line)?;
            let s =
            if condition {
                format!("[{path}] Check passed: {description}")
            } else {
                format!("[{path}] Check failed: {description}")
            };
            push_log(lua, MessageType::MessageInfo, &s);
            let wa = WorldAccess::fetch_readonly(lua);
            let mut commands = wa.access_commands();
            commands.push_entry(script, MessageType::MessageInfo, path, s.clone());
            drop(commands);
            let signal = TestServiceMembers::fetch_members(&wa.access_read_only(), this.entity()).server_collect_conditional_result.reference();
            drop(wa);
            signal.fire_in_lua(lua, false, (condition, description, script.map(|e| ObjectRef::new(lua, e)), real_line))
        }
        fn checkpoint(lua: &Lua, this: ObjectRef, text: String, source: Option<ObjectRef>, line: Option<usize>) -> LuaResult<()> {
            let (script, path, real_line) = calculate_path(lua, source, line)?;
            let s = format!("[{path}] Test checkpoint: {text}");
            push_log(lua, MessageType::MessageInfo, &s);
            let wa = WorldAccess::fetch_readonly(lua);
            let mut commands = wa.access_commands();
            commands.push_entry(script, MessageType::MessageInfo, path, s);
            drop(commands);
            let signal = TestServiceMembers::fetch_members(&wa.access_read_only(), this.entity()).server_collect_result.reference();
            drop(wa);
            signal.fire_in_lua(lua, false, (text, script.map(|e| ObjectRef::new(lua, e)), real_line))
        }
        fn done(lua: &Lua, this: ObjectRef) -> LuaResult<()> {
            let (script, path, _) = calculate_path(lua, None, None)?;
            push_log(lua, MessageType::MessageInfo, "Testing Done");
            let wa = WorldAccess::fetch_readonly(lua);
            let mut commands = wa.access_commands();
            commands.push_entry(script, MessageType::MessageInfo, path, String::from("Testing Done"));
            drop(commands);
            let world = wa.access_read_only();
            let event = TestServiceMembers::fetch_members(&world, this.entity()).done.reference();
            drop(world);
            drop(wa);
            event.fire_in_lua(lua, false, ())?;
            Ok(())
        }
        fn error(lua: &Lua, this: ObjectRef, description: String, source: Option<ObjectRef>, line: Option<usize>) -> LuaResult<()> {
            let (script, path, real_line) = calculate_path(lua, source, line)?;
            let s = format!("[{path}] TestService: {description}");
            push_log(lua, MessageType::MessageError, &s);
            let wa = WorldAccess::fetch_readonly(lua);
            let mut commands = wa.access_commands();
            commands.push_entry(script, MessageType::MessageError, path, s);
            drop(commands);
            let signal = TestServiceMembers::fetch_members(&wa.access_read_only(), this.entity()).server_collect_result.reference();
            drop(wa);
            signal.fire_in_lua(lua, false, (description, script.map(|e| ObjectRef::new(lua, e)), real_line))
        }
        fn fail(lua: &Lua, this: ObjectRef, description: String, source: Option<ObjectRef>, line: Option<usize>) -> LuaResult<()> {
            let (script, path, real_line) = calculate_path(lua, source, line)?;
            let s = format!("[{path}] Fatal error. Test ended: {description}");
            push_log(lua, MessageType::MessageError, &s);
            let wa = WorldAccess::fetch_readonly(lua);
            let mut commands = wa.access_commands();
            commands.push_entry(script, MessageType::MessageError, path, s);
            drop(commands);
            let world = wa.access_read_only();
            let members = TestServiceMembers::fetch_members(&world, this.entity());
            let signal = members.server_collect_result.reference();
            let done = members.done.reference();
            drop(world);
            drop(wa);
            signal.fire_in_lua(lua, false, (description.clone(), script.map(|e| ObjectRef::new(lua, e)), real_line))?;
            done.fire_in_lua(lua, false, ())?;
            Err(LuaError::external(description))
        }
        fn is_feature_enabled(lua: &Lua, this: ObjectRef, name: String) -> LuaResult<bool> {
            Ok(match name.as_str() {
                "deprecated" => cfg!(feature="deprecated"),
                "multiplayer" => cfg!(feature="multiplayer"),
                "bevy" => true,
                _ => return Err(LuaError::runtime("unknown feature"))
            })
        }
        fn message(lua: &Lua, this: ObjectRef, text: String, source: Option<ObjectRef>, line: Option<usize>) -> LuaResult<()> {
            let (script, path, real_line) = calculate_path(lua, source, line)?;
            let s = format!("[{path}] TestService: {text}");
            push_log(lua, MessageType::MessageInfo, &s);
            let wa = WorldAccess::fetch_readonly(lua);
            let mut commands = wa.access_commands();
            commands.push_entry(script, MessageType::MessageInfo, path, s);
            drop(commands);
            let signal = TestServiceMembers::fetch_members(&wa.access_read_only(), this.entity()).server_collect_result.reference();
            drop(wa);
            signal.fire_in_lua(lua, false, (text, script.map(|e| ObjectRef::new(lua, e)), real_line))?;
            Ok(())
        }
        fn register_test(lua: &Lua, this: ObjectRef, _test_options: LuaTable) -> LuaResult<ObjectRef> {
            lua_todo!()
        }
        fn require(lua: &Lua, this: ObjectRef, condition: bool, description: String, source: Option<ObjectRef>, line: Option<usize>) -> LuaResult<()> {
            let (script, path, real_line) = calculate_path(lua, source, line)?;
            let ty;
            let s =
            if condition {
                ty = MessageType::MessageInfo;
                format!("[{path}] Require passed: {description}")
            } else {
                ty = MessageType::MessageError;
                format!("[{path}] Require failed. Test ended: {description}")
            };
            push_log(lua, ty, &s);
            let wa = WorldAccess::fetch_readonly(lua);
            let mut commands = wa.access_commands();
            commands.push_entry(script, ty, path, s);
            drop(commands);
            let world = wa.access_read_only();
            let members = TestServiceMembers::fetch_members(&world, this.entity());
            let signal = members.server_collect_conditional_result.reference();
            let done = if !condition {
                Some(members.done.reference())
            } else {
                None
            };
            drop(world);
            drop(wa);
            signal.fire_in_lua(lua, false, (condition, description.clone(), script.map(|e| ObjectRef::new(lua, e)), real_line))?;
            if condition {
                Ok(())
            } else {
                done.unwrap().fire_in_lua(lua, false, ())?;
                Err(LuaError::external(description))
            }
        }
        #[deprecated_alias="Run"]
        #[security=PLUGIN]
        async fn run_async(lua: Lua, this: ObjectRef) -> LuaResult<bool> {
            if !TaskScheduler::can_yield(&lua) {
                return Err(LuaError::runtime("cannot yield in current context"))
            }
            let done;
            let timeout_secs;
            let mut refs = Vec::new();
            {
                let mut wa = WorldAccess::fetch(&lua);
                let world = wa.access_synchronized()?;
                // assert main thread
                if world.query_filtered::<&LuauContainer, With<DataModelMembers>>().single(world).unwrap().lua.weak() != lua.weak() {
                    return Err(LuaError::runtime("cannot run outside of main lua instance"));
                }
                let mut members = TestServiceMembers::fetch_members_mut(world, this.entity());
                done = members.done.reference();
                if members.running {
                    return Err(LuaError::runtime("tests already running"));
                }
                members.running = true;
                timeout_secs = members.timeout;
                drop(members);
                let children = world.get::<Children>(this.entity()).map(|v| v.iter().copied().collect::<Vec<_>>()).unwrap_or_default();
                for e in children {
                    let mut world_entity = world.entity_mut(e);
                    if world_entity.get::<BaseScriptMembers>().is_some() && world_entity.get::<DisabledObject>().is_some() {
                        world_entity.remove::<DisabledObject>();
                        drop(world_entity);
                        refs.push(ObjectRef::new_world(world, e));
                    }
                }
                let mut members = TestServiceMembers::fetch_members_mut(world, this.entity());
                members.test_count = refs.len() as u64;
                drop(members);
                drop(wa);
                for object in &refs {
                    let v = object.clone().into_lua(&lua)?;
                    let u = v.as_userdata().unwrap();
                    if u.get::<bool>("Enabled")? {
                        u.set("Enabled", false)?;
                        u.set("Enabled", true)?;
                    }
                }
            }
            let done_clone = done.reference();
            let timeout_thread = if timeout_secs == 0.0 {
                lua.create_thread(lua.create_function(|_, ()| Ok (()))?)?
            } else {
                TaskScheduler::fetch(&lua)
                .delay(
                    &lua,
                    Duration::from_secs_f64(timeout_secs),
                    lua.create_function(move |lua: &Lua, ()| done_clone.fire_in_lua(lua, false, ()))?,
                    ()
                )?
            };
            let auto_finish = TaskScheduler::fetch(&lua)
                .delay(
                    &lua,
                    Duration::from_millis(100),
                    lua.create_async_function(async move |lua: Lua, (this, refs): (ObjectRef, Vec<ObjectRef>)| {
                        let done_clone = TestServiceMembers::fetch_members_mut(WorldAccess::fetch(&lua).access_synchronized()?, this.entity()).done.reference();
                        // busy wait
                        loop {
                            let mut done = true;
                            'refs: for e in refs.iter() {
                                for t in ThreadIdentity::get_threads(&lua, e.entity()) {
                                    match t.status() {
                                        LuaThreadStatus::Resumable |
                                        LuaThreadStatus::Running => {
                                            done = false;
                                            break 'refs;
                                        }
                                        _ => ()
                                    }
                                }
                            }
                            if done {
                                done_clone.fire_in_lua(&lua, false, ())?;
                                return Ok(());
                            }
                            TaskScheduler::fetch(&lua).defer_next_frame(&lua, lua.current_thread(), ())?;
                            lua.yield_with::<()>(()).await?;
                        }
                    })?,
                    (this.clone(), refs)
                )?;

            done.wait::<()>(&lua).await?;

            TaskScheduler::fetch(&lua).cancel(&lua, timeout_thread)?;
            TaskScheduler::fetch(&lua).cancel(&lua, auto_finish)?;

            let mut wa = WorldAccess::fetch(&lua);
            let world = wa.access_synchronized()?;
            let mut members = TestServiceMembers::fetch_members_mut(world, this.entity());
            members.running = false;
            drop(members);

            let mut logs = world.remove_resource::<TestServiceLogs>().unwrap();
            drop(wa);

            let tests = logs.fetch_failed_tests();
            let success = if !tests.is_empty() {
                push_log(&lua, MessageType::MessageError, format!("tests failed, see below for logs"));
                for test in tests {
                    logs = logs.print_logs(&lua, test);
                }
                false
            } else {
                true
            };
            let mut wa = WorldAccess::fetch(&lua);
            let world = wa.access_synchronized()?;
            world.insert_resource(logs);
            let children = world.get::<Children>(this.entity()).map(|v| v.iter().copied().collect::<Vec<_>>()).unwrap_or_default();
            for e in children {
                world.entity_mut(e).insert(DisabledObject);
            }
            Ok(success)

        }
        fn scope_time(lua: &Lua, this: ObjectRef) -> LuaResult<LuaTable> {
            lua_unimplemented!("we have no use for this")
        }
        fn take_snapshot(lua: &Lua, this: ObjectRef, _name: String, _source: Option<ObjectRef>) -> LuaResult<()> {
            lua_todo!()
        }
        fn warn(lua: &Lua, this: ObjectRef, condition: bool, description: String, source: Option<ObjectRef>, line: Option<usize>) -> LuaResult<()> {
            let (script, path, real_line) = calculate_path(lua, source, line)?;
            let ty;
            let s =
            if condition {
                ty = MessageType::MessageInfo;
                format!("[{path}] Warning passed: {description}")
            } else {
                ty = MessageType::MessageWarning;
                format!("[{path}] Warning: {description}")
            };
            push_log(lua, ty, &s);
            let wa = WorldAccess::fetch_readonly(lua);
            let mut commands = wa.access_commands();
            commands.push_entry(script, ty, path, s);
            drop(commands);
            let signal = TestServiceMembers::fetch_members(&wa.access_read_only(), this.entity()).server_collect_conditional_result.reference();
            drop(wa);
            signal.fire_in_lua(lua, false, (condition, description, script.map(|e| ObjectRef::new(lua, e)), real_line))
        }
    }
}

pub fn insert_test_service_macros(source: String) -> Result<String, LuaError> {
    fn take_arg<'it, 'buf>(
        source: &'it mut Peekable<impl Iterator<Item = char>>,
        buffer: &'buf mut String,
    ) -> Result<&'buf str, LuaError> {
        let mut paren_content = 0;
        let l = buffer.len();
        while let Some(c) = source.peek().cloned() {
            if (c == ',' || c == ')') && paren_content == 0 {
                return Ok(&buffer[l..]);
            }
            source.next();
            match c {
                '(' => paren_content += 1,
                ')' => paren_content -= 1,
                c => buffer.push(c),
            }
        }
        Err(LuaError::SyntaxError {
            message: "unexpected EOF whie parsing test service macro".to_string(),
            incomplete_input: true,
        })
    }
    fn macro_replace(
        source: &mut Peekable<impl Iterator<Item = char>>,
        ident_buffer: String,
        buffer: &mut String,
    ) -> Result<(), LuaError> {
        let end_part;
        // start part
        match ident_buffer.as_str() {
            _ if ident_buffer.starts_with("RBX_CHECK") => {
                buffer.push_str("game:GetService('TestService'):Check");
                end_part = &ident_buffer["RBX_CHECK".len()..];
            }
            _ if ident_buffer.starts_with("RBX_REQUIRE") => {
                buffer.push_str("game:GetService('TestService'):Require");
                end_part = &ident_buffer["RBX_REQUIRE".len()..];
            }
            _ if ident_buffer.starts_with("RBX_WARN") => {
                buffer.push_str("game:GetService('TestService'):Warn");
                end_part = &ident_buffer["RBX_WARN".len()..];
            }
            _ => unreachable!(),
        }
        buffer.push('(');
        match end_part {
            "" => {
                let s = take_arg(source, buffer)?;
                if source.peek().cloned() == Some(')') {
                    source.next();
                    let s = format!(",{s:?},script)");
                    buffer.push_str(&s);
                } else {
                    return Err(LuaError::SyntaxError {
                        message: "expected ')'".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                }
            }
            "_MESSAGE" => {
                take_arg(source, buffer)?;
                if source.peek().cloned() != Some(',') {
                    return Err(LuaError::SyntaxError {
                        message: "expected ','".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                } else {
                    source.next();
                }
                let msg = take_arg(source, buffer)?;
                if source.peek().cloned() == Some(')') {
                    source.next();
                    let s = format!(",{msg},script)");
                    buffer.push_str(&s);
                } else {
                    return Err(LuaError::SyntaxError {
                        message: "expected ')'".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                }
            }
            "_THROW" => {
                buffer.push_str("pcall(function() ");
                let code = take_arg(source, buffer)?;
                if source.peek().cloned() == Some(')') {
                    source.next();
                    let s = format!(
                        " end) == false,\"expected to throw an error: {}\",script)",
                        code.escape_debug()
                    );
                    buffer.push_str(&s);
                } else {
                    return Err(LuaError::SyntaxError {
                        message: "expected ')'".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                }
            }

            "_NO_THROW" => {
                buffer.push_str("pcall(function() ");
                let code = take_arg(source, buffer)?;
                if source.peek().cloned() == Some(')') {
                    source.next();
                    let s = format!(
                        " end) == true,\"expected to not throw an error: {}\",script)",
                        code.escape_debug()
                    );
                    buffer.push_str(&s);
                } else {
                    return Err(LuaError::SyntaxError {
                        message: "expected ')'".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                }
            }
            "_EQUAL" | "_EQ" => {
                let a = take_arg(source, buffer)?.to_string();
                if source.peek().cloned() != Some(',') {
                    return Err(LuaError::SyntaxError {
                        message: "expected ','".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                } else {
                    source.next();
                }
                buffer.push_str(" == ");
                let b = take_arg(source, buffer)?.to_string();
                if source.peek().cloned() == Some(')') {
                    source.next();
                    buffer.push_str(&format!(
                        ",\"{} == {}\",script)",
                        a.escape_debug(),
                        b.escape_debug()
                    ));
                } else {
                    return Err(LuaError::SyntaxError {
                        message: "expected ')'".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                }
            }
            "_NE" => {
                let a = take_arg(source, buffer)?.to_string();
                if source.peek().cloned() != Some(',') {
                    return Err(LuaError::SyntaxError {
                        message: "expected ','".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                } else {
                    source.next();
                }
                buffer.push_str(" ~= ");
                let b = take_arg(source, buffer)?.to_string();
                if source.peek().cloned() == Some(')') {
                    source.next();
                    buffer.push_str(&format!(
                        ",\"{} ~= {}\",script)",
                        a.escape_debug(),
                        b.escape_debug()
                    ));
                } else {
                    return Err(LuaError::SyntaxError {
                        message: "expected ')'".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                }
            }
            "_GE" => {
                let a = take_arg(source, buffer)?.to_string();
                if source.peek().cloned() != Some(',') {
                    return Err(LuaError::SyntaxError {
                        message: "expected ','".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                } else {
                    source.next();
                }
                buffer.push_str(" >= ");
                let b = take_arg(source, buffer)?.to_string();
                if source.peek().cloned() == Some(')') {
                    source.next();
                    buffer.push_str(&format!(
                        ",\"{} >= {}\",script)",
                        a.escape_debug(),
                        b.escape_debug()
                    ));
                } else {
                    return Err(LuaError::SyntaxError {
                        message: "expected ')'".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                }
            }
            "_LE" => {
                let a = take_arg(source, buffer)?.to_string();
                if source.peek().cloned() != Some(',') {
                    return Err(LuaError::SyntaxError {
                        message: "expected ','".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                } else {
                    source.next();
                }
                buffer.push_str(" <= ");
                let b = take_arg(source, buffer)?.to_string();
                if source.peek().cloned() == Some(')') {
                    source.next();
                    buffer.push_str(&format!(
                        ",\"{} <= {}\",script)",
                        a.escape_debug(),
                        b.escape_debug()
                    ));
                } else {
                    return Err(LuaError::SyntaxError {
                        message: "expected ')'".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                }
            }
            "_GT" => {
                let a = take_arg(source, buffer)?.to_string();
                if source.peek().cloned() != Some(',') {
                    return Err(LuaError::SyntaxError {
                        message: "expected ','".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                } else {
                    source.next();
                }
                buffer.push_str(" > ");
                let b = take_arg(source, buffer)?.to_string();
                if source.peek().cloned() == Some(')') {
                    source.next();
                    buffer.push_str(&format!(
                        ",\"{} > {}\",script)",
                        a.escape_debug(),
                        b.escape_debug()
                    ));
                } else {
                    return Err(LuaError::SyntaxError {
                        message: "expected ')'".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                }
            }
            "_LT" => {
                let a = take_arg(source, buffer)?.to_string();
                if source.peek().cloned() != Some(',') {
                    return Err(LuaError::SyntaxError {
                        message: "expected ','".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                } else {
                    source.next();
                }
                buffer.push_str(" < ");
                let b = take_arg(source, buffer)?.to_string();
                if source.peek().cloned() == Some(')') {
                    source.next();
                    buffer.push_str(&format!(
                        ",\"{} < {}\",script)",
                        a.escape_debug(),
                        b.escape_debug()
                    ));
                } else {
                    return Err(LuaError::SyntaxError {
                        message: "expected ')'".to_string(),
                        incomplete_input: source.next().is_none(),
                    });
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    let mut new_source = String::new();

    let mut ident_buffer = String::new();
    let mut chars = source.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            a if a.is_alphanumeric() || a == '_' => {
                new_source.push(a);
                ident_buffer.push(a);
            }
            '(' => match ident_buffer.as_str() {
                "RBX_ERROR" => {
                    new_source = new_source[..new_source.len() - "RBX_ERROR".len()].to_string();
                    new_source.push_str("game:GetService('TestService'):Error");
                    new_source.push('(');
                    ident_buffer.clear();
                }
                "RBX_FAIL" => {
                    new_source = new_source[..new_source.len() - "RBX_FAIL".len()].to_string();
                    new_source.push_str("game:GetService('TestService'):Fail");
                    new_source.push('(');
                    ident_buffer.clear();
                }
                "RBX_MESSAGE" => {
                    new_source = new_source[..new_source.len() - "RBX_MESSAGE".len()].to_string();
                    new_source.push_str("game:GetService('TestService'):Message");
                    new_source.push('(');
                    ident_buffer.clear();
                }
                "RBX_CHECK"
                | "RBX_CHECK_MESSAGE"
                | "RBX_CHECK_THROW"
                | "RBX_CHECK_NO_THROW"
                | "RBX_CHECK_EQUAL"
                | "RBX_CHECK_EQ"
                | "RBX_CHECK_NE"
                | "RBX_CHECK_GE"
                | "RBX_CHECK_LE"
                | "RBX_CHECK_GT"
                | "RBX_CHECK_LT"
                | "RBX_REQUIRE"
                | "RBX_REQUIRE_MESSAGE"
                | "RBX_REQUIRE_THROW"
                | "RBX_REQUIRE_NO_THROW"
                | "RBX_REQUIRE_EQUAL"
                | "RBX_REQUIRE_EQ"
                | "RBX_REQUIRE_NE"
                | "RBX_REQUIRE_GE"
                | "RBX_REQUIRE_LE"
                | "RBX_REQUIRE_GT"
                | "RBX_REQUIRE_LT"
                | "RBX_WARN"
                | "RBX_WARN_MESSAGE"
                | "RBX_WARN_THROW"
                | "RBX_WARN_NO_THROW"
                | "RBX_WARN_EQUAL"
                | "RBX_WARN_EQ"
                | "RBX_WARN_NE"
                | "RBX_WARN_GE"
                | "RBX_WARN_LE"
                | "RBX_WARN_GT"
                | "RBX_WARN_LT" => {
                    new_source = new_source[..new_source.len() - ident_buffer.len()].to_string();
                    macro_replace(
                        &mut chars,
                        std::mem::take(&mut ident_buffer),
                        &mut new_source,
                    )?;
                }
                _ => new_source.push('('),
            },
            c => {
                ident_buffer.clear();
                new_source.push(c);
            }
        }
    }
    Ok(new_source)
}
