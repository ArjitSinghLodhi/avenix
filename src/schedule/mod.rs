use crate::{
    app::{App, plugin::Plugin},
    extensions::SystemExt,
};

mod schedules_list;

pub use schedules_list::{CleanupHandles, First, Last, PostUpdate, PreUpdate, Startup, Update};

use crate::extensions::{System, World};

use std::{
    any::{Any, TypeId},
    fmt::Debug,
};

pub(crate) struct DefaultSchedulesPlugin;

impl Plugin for DefaultSchedulesPlugin {
    fn build(self, app: &mut App) {
        app.add_schedule(Schedule::new(First))
            .add_schedule(Schedule::new(PreUpdate))
            .add_schedule(Schedule::new(Update))
            .add_schedule(Schedule::new(PostUpdate))
            .add_schedule(Schedule::new(Last));

        app.configure_schedule_order(First, PreUpdate)
            .configure_schedule_order(PreUpdate, Update)
            .configure_schedule_order(Update, PostUpdate)
            .configure_schedule_order(PostUpdate, Last);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScheduleId {
    pub(crate) id: TypeId,
    pub(crate) name: &'static str,
}

pub trait IntoScheduleId: ScheduleLabel {
    fn id(&self) -> ScheduleId
    where
        Self: Sized,
    {
        ScheduleId {
            id: TypeId::of::<Self>(),
            name: std::any::type_name::<Self>(),
        }
    }
}

impl<T: ?Sized + ScheduleLabel> IntoScheduleId for T {}

pub trait ScheduleLabel: Any + Send + Sync {
    fn default_executor(&mut self) -> Box<dyn SystemExecutor> {
        Box::new(SingleThreadedExecutor)
    }
}

pub type ConditionFn = Box<dyn Fn(&World) -> bool>;

#[derive(Default)]
pub struct RunConditionsList {
    pub(crate) run_conditions: Vec<ConditionFn>,
}

impl RunConditionsList {
    pub fn conditions(&self) -> &Vec<ConditionFn> {
        &self.run_conditions
    }

    pub fn conditions_mut(&mut self) -> &mut Vec<ConditionFn> {
        &mut self.run_conditions
    }
}

#[doc(hidden)]
pub struct SystemNode {
    system: Box<dyn System>,
}

impl SystemNode {
    pub fn new(system: Box<dyn System>) -> Self {
        Self { system }
    }

    pub fn run(&mut self, world: &mut World) {
        self.system.run(world);
    }
}

pub struct SystemsSchedule {
    systems: Vec<SystemNode>,
}

impl SystemsSchedule {
    pub(crate) fn new() -> Self {
        Self {
            systems: Vec::new(),
        }
    }

    pub fn systems_mut(&mut self) -> impl Iterator<Item = &mut SystemNode> {
        self.systems.iter_mut()
    }
}

pub trait SystemExecutor: Send + Sync {
    fn run(&mut self, schedule: &mut SystemsSchedule, world: &mut World);
}

#[derive(Default)]
pub struct SingleThreadedExecutor;

impl SystemExecutor for SingleThreadedExecutor {
    fn run(&mut self, schedule: &mut SystemsSchedule, world: &mut World) {
        for node in schedule.systems_mut() {
            let should_run = node
                .system
                .get_or_init(RunConditionsList::default)
                .run_conditions
                .iter()
                .all(|cond| cond(world));

            if should_run {
                node.system.run(world);
            }
        }
    }
}

#[doc(hidden)]
pub struct Schedule {
    id: ScheduleId,
    systems_schedule: SystemsSchedule,
    executor: Box<dyn SystemExecutor>,
}

impl Schedule {
    pub fn new<L: ScheduleLabel + 'static>(mut label: L) -> Self {
        Self {
            id: label.id(),
            executor: label.default_executor(),
            systems_schedule: SystemsSchedule::new(),
        }
    }

    pub fn id(&self) -> ScheduleId {
        self.id
    }

    pub fn set_executor(&mut self, executor: impl SystemExecutor + 'static) {
        self.executor = Box::new(executor);
    }

    pub fn add_system(&mut self, system: Box<dyn System>) {
        self.systems_schedule.systems.push(SystemNode::new(system));
    }

    pub fn run(&mut self, world: &mut World) {
        self.executor.run(&mut self.systems_schedule, world);
    }
}
