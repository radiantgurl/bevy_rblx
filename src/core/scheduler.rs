use std::{
    cell::{Cell, RefCell},
    mem::take,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crate::core::{FAST_FLAGS, LuaSingleton, logs::push_lua_error};
use bevy::prelude::*;
use bevy_rblx_derive::{fast_flag, register};
use mlua::{AppDataRef, prelude::*};

use crate::internal_prelude::*;

#[derive(Default)]
struct InternalTaskScheduler {
    defer_threads: [Vec<(LuaThread, LuaMultiValue)>; 2],
    defer_next_threads: [Vec<(LuaThread, LuaMultiValue)>; 2],
    delay_threads: [Vec<(LuaThread, Instant, Duration, LuaMultiValue)>; 2],

    wait_threads: [Vec<(LuaThread, Instant, Duration)>; 2],

    parallel_dispatch: bool,
}

#[derive(Clone)]
struct EmptyCompiled(LuaFunction);

#[derive(Default)]
pub struct TaskScheduler {
    cell: RefCell<InternalTaskScheduler>,

    watchdog: Cell<Option<Instant>>,
    early_interrupt: Arc<AtomicBool>,
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
        if let Err(e) = t.resume::<()>(values) {
            push_lua_error(lua, t.clone(), e);
        }
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
    pub fn defer_custom_pd(
        &self,
        lua: &Lua,
        t: impl IntoLuaThread,
        values: impl IntoLuaMulti,
        pd: bool,
    ) -> LuaResult<LuaThread> {
        let mut task = self.cell.borrow_mut();
        let t = t.into_lua_thread(lua)?;
        task.defer_threads[pd as usize].push((t.clone(), values.into_lua_multi(lua)?));
        Ok(t)
    }
    pub fn defer_high_priority(
        &self,
        lua: &Lua,
        t: impl IntoLuaThread,
        values: impl IntoLuaMulti,
    ) -> LuaResult<LuaThread> {
        let mut task = self.cell.borrow_mut();
        let pd = task.parallel_dispatch as usize;
        let t = t.into_lua_thread(lua)?;
        task.defer_threads[pd].insert(0, (t.clone(), values.into_lua_multi(lua)?));
        Ok(t)
    }
    pub fn defer_next_frame(
        &self,
        lua: &Lua,
        t: impl IntoLuaThread,
        values: impl IntoLuaMulti,
    ) -> LuaResult<LuaThread> {
        let mut task = self.cell.borrow_mut();
        let pd = task.parallel_dispatch as usize;
        let t = t.into_lua_thread(lua)?;
        task.defer_next_threads[pd].push((t.clone(), values.into_lua_multi(lua)?));
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
    pub fn is_desynchronized(&self) -> bool {
        self.cell.borrow().parallel_dispatch
    }
    fn spawn_lua(lua: &Lua, mut values: LuaMultiValue) -> LuaResult<LuaThread> {
        let task = lua.app_data_ref::<TaskScheduler>().unwrap();
        let ft = values.pop_front().unwrap_or_default();
        task.spawn(lua, ft, values)
    }
    fn defer_lua(lua: &Lua, mut values: LuaMultiValue) -> LuaResult<LuaThread> {
        let task = lua.app_data_ref::<TaskScheduler>().unwrap();
        let ft = values.pop_front().unwrap_or_default();
        task.defer_next_frame(lua, ft, values)
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
    #[cfg(feature = "deprecated")]
    fn spawn_deprecated_lua(lua: &Lua, mut values: LuaMultiValue) -> LuaResult<LuaThread> {
        let task = lua.app_data_ref::<TaskScheduler>().unwrap();
        let ft = values.pop_front().unwrap_or_default();
        task.defer_next_frame(lua, ft, values)
    }
    #[cfg(feature = "deprecated")]
    async fn wait_deprecated_lua(lua: Lua, (delay,): (f64,)) -> LuaResult<f64> {
        let start = Instant::now();
        let new_duration = Duration::from_secs_f64(delay);
        loop {
            {
                let task = lua
                    .app_data_ref::<TaskScheduler>()
                    .expect("expected task scheduler to be init");
                task.defer_next_frame(&lua, lua.current_thread(), ())?;
            }
            lua.yield_with::<()>(()).await?;
            if start.elapsed() >= new_duration {
                return Ok(start.elapsed().as_secs_f64());
            }
        }
    }

    fn watchdog_check(&self) -> bool {
        (self.watchdog.get())
            .expect("task scheduler is initialized and is currently executing this thread")
            .checked_duration_since(Instant::now())
            .is_some()
    }
    pub(crate) fn run(
        &self,
        lua: &Lua,
        parallel_dispatch: bool,
        new_frame: bool,
        allocated_duration: Duration,
        hard_limit_duration: Option<Duration>,
    ) -> bool {
        assert!(
            self.watchdog.get().is_none(),
            "expected to not attempt running task scheduler inside itself"
        );
        let pd = parallel_dispatch as usize;
        let start = Instant::now();

        self.early_interrupt.store(false, Ordering::Relaxed);

        unsafe {
            self.start_watchdog(hard_limit_duration);
            if !FAST_FLAGS.fetch::<FFTaskSchedulerDisableWatchdog>() {
                lua.set_interrupt(TaskScheduler::watchdog);
            }
        }

        self.cell.borrow_mut().parallel_dispatch = parallel_dispatch;

        if new_frame {
            let defer_new_frame_threads = take(&mut self.cell.borrow_mut().defer_next_threads[pd]);
            for (t, v) in defer_new_frame_threads {
                if t.status() == LuaThreadStatus::Resumable {
                    if let Err(e) = t.resume::<()>(v) {
                        push_lua_error(lua, t, e);
                    }
                }
            }
        }

        let mut repeat = true;
        while repeat && start.elapsed() < allocated_duration {
            let defer_threads = take(&mut self.cell.borrow_mut().defer_threads[pd]);
            for (t, v) in defer_threads {
                if t.status() == LuaThreadStatus::Resumable {
                    if let Err(e) = t.resume::<()>(v) {
                        push_lua_error(lua, t, e);
                    }
                }
            }

            repeat &= self.cell.borrow().defer_threads[pd].len() > 0;

            if FAST_FLAGS.fetch::<FFTaskSchedulerTimeSensitive>() || new_frame {
                let mut still_waiting_delay = Vec::new();
                let new_delay_threads = take(&mut self.cell.borrow_mut().delay_threads[pd]);
                for (t, i, d, v) in new_delay_threads {
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
                self.cell.borrow_mut().delay_threads[pd].append(&mut still_waiting_delay);

                let mut still_waiting_wait = Vec::new();
                let new_waiting_threads = take(&mut self.cell.borrow_mut().wait_threads[pd]);
                for (t, i, d) in new_waiting_threads {
                    if t.status() == LuaThreadStatus::Resumable {
                        if Instant::now().duration_since(i) >= d {
                            if let Err(e) =
                                t.resume::<()>(Instant::now().duration_since(i).as_secs_f64())
                            {
                                push_lua_error(lua, t, e);
                            }
                        } else {
                            still_waiting_wait.push((t, i, d));
                        }
                    }
                }
                self.cell.borrow_mut().wait_threads[pd].append(&mut still_waiting_wait);
            }

            repeat &= !self.early_interrupt.load(Ordering::Relaxed)
        }
        unsafe {
            lua.remove_interrupt();
            self.stop_watchdog();
        }
        self.early_interrupt.load(Ordering::Relaxed)
    }
    pub fn fetch(lua: &Lua) -> AppDataRef<'_, TaskScheduler> {
        lua.app_data_ref::<TaskScheduler>()
            .expect("task scheduler is initialized")
    }

    pub unsafe fn start_watchdog(&self, hard_limit_duration: Option<Duration>) {
        let start = Instant::now();
        let hard_limit_duration = hard_limit_duration.unwrap_or(Duration::from_secs(10));
        let watchdog_time = start
            .checked_add(hard_limit_duration)
            .expect("expected no bugs in time system");

        self.watchdog.set(Some(watchdog_time));
    }
    pub unsafe fn stop_watchdog(&self) {
        self.watchdog.set(None);
    }

    pub fn get_early_interrupt_flag(&self) -> Weak<AtomicBool> {
        Arc::downgrade(&self.early_interrupt)
    }

    fn watchdog(lua: &Lua) -> LuaResult<LuaVmState> {
        if lua
            .app_data_ref::<Self>()
            .expect("task scheduler is init")
            .watchdog_check()
        {
            Ok(LuaVmState::Continue)
        } else {
            Err(LuaError::runtime("script exhausted execution time"))
        }
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

        lua.globals().raw_set("task", task)?;

        #[cfg(feature = "deprecated")]
        {
            lua.globals().raw_set(
                "spawn",
                lua.create_function(TaskScheduler::spawn_deprecated_lua)?,
            )?;
            lua.globals().raw_set(
                "wait",
                lua.create_function(TaskScheduler::wait_deprecated_lua)?,
            )?;
        }

        Ok(())
    }
}

fast_flag!(FFTaskSchedulerDisableWatchdog: bool = false);
fast_flag!(FFTaskSchedulerTimeSensitive: bool = false);
