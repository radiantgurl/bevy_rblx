use std::{
    cell::RefCell,
    mem::take,
    time::{Duration, Instant},
};

use crate::core::{LuaSingleton, logs::push_lua_error};
use bevy::prelude::*;
use bevy_rblx_derive::register;
use mlua::prelude::*;

use crate::internal_prelude::*;

#[derive(Default)]
struct InternalTaskScheduler {
    defer_threads: [Vec<(LuaThread, LuaMultiValue)>; 2],
    delay_threads: [Vec<(LuaThread, Instant, Duration, LuaMultiValue)>; 2],

    wait_threads: [Vec<(LuaThread, Instant, Duration)>; 2],

    parallel_dispatch: bool,
}

#[derive(Clone)]
struct EmptyCompiled(LuaFunction);

#[derive(Default)]
pub struct TaskScheduler {
    cell: RefCell<InternalTaskScheduler>,
}

const fn empty(_: &Lua, _: ()) -> LuaResult<()> {
    Ok(())
}

impl TaskScheduler {
    pub fn spawn(
        &self,
        lua: &Lua,
        t: impl IntoLuaThread,
        values: impl IntoLuaMulti,
    ) -> LuaResult<LuaThread> {
        let t = t.into_lua_thread(lua)?;
        let _: () = t.resume(values)?;
        Ok(t)
    }

    pub fn defer(
        &self,
        lua: &Lua,
        t: impl IntoLuaThread,
        values: impl IntoLuaMulti,
    ) -> LuaResult<LuaThread> {
        let mut task = self.cell.borrow_mut();
        let pd = task.parallel_dispatch as usize;
        let t = t.into_lua_thread(lua)?;
        task.defer_threads[pd].push((t.clone(), values.into_lua_multi(lua)?));
        Ok(t)
    }

    pub fn delay(
        &self,
        lua: &Lua,
        t: impl IntoLuaThread,
        delay: Duration,
        values: impl IntoLuaMulti,
    ) -> LuaResult<LuaThread> {
        let mut task = self.cell.borrow_mut();
        let pd = task.parallel_dispatch as usize;
        let t = t.into_lua_thread(lua)?;
        task.delay_threads[pd].push((
            t.clone(),
            Instant::now(),
            delay,
            values.into_lua_multi(lua)?,
        ));
        Ok(t)
    }

    pub async fn wait(&self, lua: &Lua, delay: Duration) -> LuaResult<f64> {
        {
            let mut task = self.cell.borrow_mut();
            let pd = task.parallel_dispatch as usize;
            task.wait_threads[pd].push((lua.current_thread(), Instant::now(), delay));
        }
        lua.yield_with(()).await
    }

    pub async fn synchronize(&self, lua: &Lua) -> LuaResult<()> {
        if self.cell.borrow().parallel_dispatch {
            self.cell.borrow_mut().defer_threads[0]
                .push((lua.current_thread(), LuaMultiValue::new()));
            lua.yield_with(()).await
        } else {
            Ok(())
        }
    }
    pub async fn desynchronize(&self, lua: &Lua) -> LuaResult<()> {
        if !self.cell.borrow().parallel_dispatch {
            self.cell.borrow_mut().defer_threads[1]
                .push((lua.current_thread(), LuaMultiValue::new()));
            lua.yield_with(()).await
        } else {
            Ok(())
        }
    }
    pub fn cancel(&self, lua: &Lua, thread: LuaThread) -> LuaResult<()> {
        match thread.status() {
            LuaThreadStatus::Resumable => {
                thread.reset(lua.app_data_ref::<EmptyCompiled>().unwrap().0.clone())
            }
            LuaThreadStatus::Running => {
                todo!()
            }
            _ => Ok(()),
        }
    }
    fn spawn_lua(lua: &Lua, mut values: LuaMultiValue) -> LuaResult<LuaThread> {
        let task = lua.app_data_ref::<TaskScheduler>().unwrap();
        let ft = values.pop_front().unwrap_or_default();
        task.spawn(lua, ft, values)
    }
    fn defer_lua(lua: &Lua, mut values: LuaMultiValue) -> LuaResult<LuaThread> {
        let task = lua.app_data_ref::<TaskScheduler>().unwrap();
        let ft = values.pop_front().unwrap_or_default();
        task.defer(lua, ft, values)
    }
    fn delay_lua(
        lua: &Lua,
        (ft, delay, values): (LuaValue, f64, LuaMultiValue),
    ) -> LuaResult<LuaThread> {
        let task = lua.app_data_ref::<TaskScheduler>().unwrap();
        task.delay(
            lua,
            ft,
            Duration::try_from_secs_f64(delay).into_lua_err()?,
            values,
        )
    }
    async fn wait_lua(lua: Lua, (delay,): (f64,)) -> LuaResult<f64> {
        {
            let t = lua.app_data_ref::<TaskScheduler>().unwrap();
            let mut task = t.cell.borrow_mut();
            let pd = task.parallel_dispatch as usize;
            task.wait_threads[pd].push((
                lua.current_thread(),
                Instant::now(),
                Duration::try_from_secs_f64(delay).into_lua_err()?,
            ));
        }
        lua.yield_with(()).await
    }
    async fn synchronize_lua(lua: Lua, _: ()) -> LuaResult<()> {
        let should_yield = {
            let task = lua.app_data_ref::<TaskScheduler>().unwrap();
            if task.cell.borrow().parallel_dispatch {
                task.cell.borrow_mut().defer_threads[0]
                    .push((lua.current_thread(), LuaMultiValue::new()));
                true
            } else {
                false
            }
        };
        if should_yield {
            lua.yield_with(()).await
        } else {
            Ok(())
        }
    }
    async fn desynchronize_lua(lua: Lua, _: ()) -> LuaResult<()> {
        let should_yield = {
            let task = lua.app_data_ref::<TaskScheduler>().unwrap();
            if !task.cell.borrow().parallel_dispatch {
                task.cell.borrow_mut().defer_threads[1]
                    .push((lua.current_thread(), LuaMultiValue::new()));
                true
            } else {
                false
            }
        };
        if should_yield {
            lua.yield_with(()).await
        } else {
            Ok(())
        }
    }
    fn cancel_lua(lua: &Lua, (thr,): (LuaThread,)) -> LuaResult<()> {
        let task = lua.app_data_ref::<TaskScheduler>().unwrap();
        task.cancel(lua, thr)
    }
    // TODO: Fix run logic
    pub(crate) fn run(&self, lua: &Lua, parallel_dispatch: bool) {
        let pd = parallel_dispatch as usize;
        self.cell.borrow_mut().parallel_dispatch = parallel_dispatch;
        for (t, v) in take(&mut self.cell.borrow_mut().defer_threads[pd]) {
            if t.status() == LuaThreadStatus::Resumable {
                if let Err(e) = t.resume::<()>(v) {
                    push_lua_error(lua, t, e);
                }
            }
        }

        let mut still_waiting_delay = Vec::new();
        for (t, i, d, v) in take(&mut self.cell.borrow_mut().delay_threads[pd]) {
            if t.status() == LuaThreadStatus::Resumable {
                if Instant::now().duration_since(i) >= d {
                    if let Err(e) = t.resume::<()>(v) {
                        push_lua_error(lua, t, e);
                    }
                } else {
                    still_waiting_delay.push((t, i, d, v));
                }
            }
        }
        self.cell.borrow_mut().delay_threads[pd] = still_waiting_delay;

        let mut still_waiting_delay = Vec::new();
        for (t, i, d) in take(&mut self.cell.borrow_mut().wait_threads[pd]) {
            if t.status() == LuaThreadStatus::Resumable {
                if Instant::now().duration_since(i) >= d {
                    if let Err(e) = t.resume::<()>(Instant::now().duration_since(i).as_secs_f64()) {
                        push_lua_error(lua, t, e);
                    }
                } else {
                    still_waiting_delay.push((t, i, d));
                }
            }
        }
        self.cell.borrow_mut().wait_threads[pd] = still_waiting_delay;
    }
}

#[register]
impl LuaSingleton for TaskScheduler {
    fn register_singleton(lua: &Lua) -> LuaResult<()> {
        lua.set_app_data(EmptyCompiled(lua.create_function(empty)?));

        let task_scheduler = Self::default();
        lua.set_app_data(task_scheduler);

        let task = lua.create_table()?;
        task.raw_set("spawn", lua.create_function(TaskScheduler::spawn_lua)?)?;
        task.raw_set("defer", lua.create_function(TaskScheduler::defer_lua)?)?;
        task.raw_set("delay", lua.create_function(TaskScheduler::delay_lua)?)?;
        task.raw_set("wait", lua.create_async_function(TaskScheduler::wait_lua)?)?;
        task.raw_set(
            "synchronize",
            lua.create_async_function(TaskScheduler::synchronize_lua)?,
        )?;
        task.raw_set(
            "desynchronize",
            lua.create_async_function(TaskScheduler::desynchronize_lua)?,
        )?;
        task.raw_set("cancel", lua.create_function(TaskScheduler::cancel_lua)?)?;

        task.set_readonly(true);

        lua.globals().raw_set("task", task)
    }
}
