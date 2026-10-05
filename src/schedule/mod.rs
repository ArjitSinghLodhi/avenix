use crate::{
    app::{App, plugin::Plugin},
    schedule::{dyn_eq::DynEq, executors::multi_threaded::MultiThreadedExecutor},
    system::condition::RunConditionsList,
};

pub(crate) mod dyn_eq;
pub(crate) mod executors;
pub(crate) mod schedule_sorter;
mod schedules_list;
pub(crate) mod system_sorter;

pub use schedules_list::{CleanupHandles, First, Last, PostUpdate, PreUpdate, Startup, Update};

use crate::extensions::{System, World};

use std::{any::Any, fmt::Debug};

pub(crate) struct DefaultSchedulesPlugin;

impl Plugin for DefaultSchedulesPlugin {
    fn build(&self, app: &mut App) {
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

pub trait ScheduleLabel: Any + Send + Sync + DynEq + Debug {
    fn default_executor(&mut self) -> Box<dyn SystemExecutor> {
        Box::new(MultiThreadedExecutor::new())
    }
}

pub struct SystemNode {
    system: Box<dyn System>,
}

unsafe impl Send for SystemNode {}
unsafe impl Sync for SystemNode {}

impl SystemNode {
    pub fn new(system: Box<dyn System>) -> Self {
        Self { system }
    }

    pub fn run(&mut self, world: &World) {
        self.system.run(world);
    }

    pub fn system(&self) -> &dyn System {
        &*self.system
    }

    pub fn system_mut(&mut self) -> &mut dyn System {
        &mut *self.system
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

    pub fn systems(&self) -> &Vec<SystemNode> {
        &self.systems
    }

    pub fn systems_mut(&mut self) -> &mut Vec<SystemNode> {
        &mut self.systems
    }
}

pub trait SystemExecutor: Send + Sync + 'static {
    fn init(&mut self, schedule: &mut SystemsSchedule);
    fn run(&mut self, schedule: &mut SystemsSchedule, world: &mut World);
}

pub struct Schedule {
    schedule: Box<dyn ScheduleLabel>,
    systems_schedule: SystemsSchedule,
    executor: Box<dyn SystemExecutor>,
}

impl Schedule {
    pub fn new<L: ScheduleLabel + 'static>(mut label: L) -> Self {
        Self {
            executor: label.default_executor(),
            schedule: Box::new(label),
            systems_schedule: SystemsSchedule::new(),
        }
    }

    pub fn set_executor(&mut self, executor: impl SystemExecutor + 'static) {
        self.executor = Box::new(executor);
    }

    pub fn add_system(&mut self, system: impl System + 'static) {
        self.systems_schedule
            .systems
            .push(SystemNode::new(Box::new(system)));
    }

    pub fn add_system_boxed(&mut self, system: Box<dyn System>) {
        self.systems_schedule.systems.push(SystemNode::new(system));
    }

    pub fn systems_schedule(&self) -> &SystemsSchedule {
        &self.systems_schedule
    }

    pub fn systems_schedule_mut(&mut self) -> &mut SystemsSchedule {
        &mut self.systems_schedule
    }

    pub fn schedule(&self) -> &dyn ScheduleLabel {
        &*self.schedule
    }

    pub fn schedule_mut(&mut self) -> &mut dyn ScheduleLabel {
        &mut *self.schedule
    }

    pub(crate) fn init_executor(&mut self) {
        self.executor.init(&mut self.systems_schedule);
    }

    pub fn run(&mut self, world: &mut World) {
        self.executor.run(&mut self.systems_schedule, world);
    }
}

pub(crate) struct ScheduleConstraint {
    pub(crate) before: Box<dyn ScheduleLabel>,
    pub(crate) after: Box<dyn ScheduleLabel>,
}
