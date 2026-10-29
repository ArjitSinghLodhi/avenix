use crate::{
    app::system::SystemExt,
    schedule::{RunConditionsList, ScheduleLabel, SystemExecutor, SystemsSchedule},
    world::World,
};

/// A schedule where queue commands and despawns are applied between each system that runs registered in this schedule.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Startup;

impl ScheduleLabel for Startup {
    fn default_executor(&mut self) -> Box<dyn SystemExecutor> {
        Box::new(StartupExecutor)
    }

    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(self.clone())
    }
}

pub(crate) struct StartupExecutor;

impl SystemExecutor for StartupExecutor {
    fn init(&mut self, _schedule: &mut SystemsSchedule) {}
    fn run(&mut self, schedule: &mut SystemsSchedule, world: &mut World) {
        for system in schedule.systems_mut() {
            let should_run = system
                .system
                .get_or_init(RunConditionsList::default)
                .conditions()
                .iter()
                .all(|cond| cond(world));
            if should_run {
                system.run(world);
            }
            world.apply_queue_commands();
            world.apply_despawns();
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct First;

impl ScheduleLabel for First {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(self.clone())
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct PreUpdate;

impl ScheduleLabel for PreUpdate {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(self.clone())
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Update;

impl ScheduleLabel for Update {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(self.clone())
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct PostUpdate;

impl ScheduleLabel for PostUpdate {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(self.clone())
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Last;

impl ScheduleLabel for Last {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(self.clone())
    }
}
