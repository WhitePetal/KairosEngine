use crate::ecs::change_detection::tick::Tick;
use crate::ecs::world::World;

#[cfg(test)]
mod test;

// ---------------------------------------------------------------------------
// SystemMeta
// ---------------------------------------------------------------------------

/// 每个 System 持有的元数据，用于 per-system 变更检测。
///
/// - `last_run`：系统上次运行时的 tick。
/// - `is_initialized`：标记 `initialize()` 是否已调用。
#[derive(Debug, Clone)]
pub struct SystemMeta {
    /// 系统上次运行时的 tick。首次运行前为"负无穷"。
    pub(crate) last_run: Tick,
    /// 是否已初始化。
    pub(crate) is_initialized: bool,
}

impl SystemMeta {
    pub fn new() -> Self {
        Self {
            last_run: Tick::MIN,
            is_initialized: false,
        }
    }
}

impl Default for SystemMeta {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// System trait
// ---------------------------------------------------------------------------

/// Bevy 风格的 System 抽象。
///
/// 每个 System 持有独立的 `last_run` tick，执行时递增 `world.change_tick` 并
/// 注入 `(last_run, this_run)` 到所有查询中，实现 per-system 变更检测。
pub trait System {
    /// 执行 system 体。
    ///
    /// 1. 递增 `world.change_tick` 得到 `this_run`
    /// 2. 将 `(meta.last_run, this_run)` 注入到 World 的查询中
    /// 3. 执行 system 体（system 体通过 `world.query_mut()` 等使用这些 ticks）
    /// 4. 清空注入，使后续查询恢复默认行为
    /// 5. 更新 `meta.last_run = this_run`
    fn run(&mut self, world: &mut World);

    /// 初始化 system。
    ///
    /// 将 `last_run` 设为"负无穷"（`change_tick.relative_to(Tick::MAX)`），
    /// 使得首次 `run()` 时所有组件都被检测为已变更/新增。
    ///
    /// 多次调用是幂等的。
    fn initialize(&mut self, world: &mut World);

    /// 返回 system 元数据的不可变引用。
    fn meta(&self) -> &SystemMeta;

    /// 返回 system 元数据的可变引用。
    fn meta_mut(&mut self) -> &mut SystemMeta;
}

// ---------------------------------------------------------------------------
// FunctionSystem — 包装 FnMut(&mut World)
// ---------------------------------------------------------------------------

/// 包装一个 `FnMut(&mut World)` 闭包的 System。
pub struct FunctionSystem<F> {
    func: F,
    meta: SystemMeta,
}

impl<F> FunctionSystem<F> {
    pub fn new(func: F) -> Self {
        Self {
            func,
            meta: SystemMeta::new(),
        }
    }
}

impl<F: FnMut(&mut World)> System for FunctionSystem<F> {
    fn run(&mut self, world: &mut World) {
        // increment_change_tick 返回旧值，需读取递增后的值作为 this_run
        world.increment_change_tick();
        let this_run = world.change_tick();
        world.set_system_ticks(self.meta.last_run, this_run);
        (self.func)(world);
        world.clear_system_ticks();
        self.meta.last_run = this_run;
    }

    fn initialize(&mut self, world: &mut World) {
        if self.meta.is_initialized {
            return;
        }
        // "负无穷"：last_run = change_tick.relative_to(Tick::MAX)
        // 使得首次 run 时，所有现有组件都被检测为已变更
        self.meta.last_run = world.change_tick().relative_to(Tick::MAX);
        self.meta.is_initialized = true;
    }

    fn meta(&self) -> &SystemMeta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut SystemMeta {
        &mut self.meta
    }
}
