use fxhash::FxHashSet;

use crate::{
    entity::Entity,
    schedule::{ScheduleLabel, SystemExecutor, SystemsSchedule},
    world::storage::World,
};

#[derive(Debug)]
pub struct Startup;

impl ScheduleLabel for Startup {
    fn default_executor(&mut self) -> Box<dyn SystemExecutor> {
        Box::new(StartupExecutor)
    }
}

pub(crate) struct StartupExecutor;

impl SystemExecutor for StartupExecutor {
    fn run(&mut self, schedule: &mut SystemsSchedule, world: &mut World) {
        for system in schedule.systems_mut() {
            let should_run = system.run_conditions.iter().all(|cond| cond(world));
            if should_run {
                system.run(world);
            }
            world.apply_commands();
        }
    }
}

pub struct First;

impl ScheduleLabel for First {}

pub struct PreUpdate;

impl ScheduleLabel for PreUpdate {}

pub struct Update;

impl ScheduleLabel for Update {}

pub struct PostUpdate;

impl ScheduleLabel for PostUpdate {}

/// A special schedule where queue commands are applied immediately before
/// and after the systems registered in this schedule run. Every system in this
/// schedule is re-run if any system issues a new despawn command. This ensures
/// every cleanup system observes all pending despawns before they are final.
/// And finally despawn commands are applied.
///
/// See [`DefaultSchedulesPlugin`] for more information on the execution order.
///
/// [`DefaultSchedulesPlugin`]: crate::schedule::DefaultSchedulesPlugin
pub struct CleanupHandles;

impl ScheduleLabel for CleanupHandles {
    fn default_executor(&mut self) -> Box<dyn SystemExecutor> {
        Box::new(CleanupHandlesExecutor {
            despawn_buffer: FxHashSet::default(),
            historical_seen: FxHashSet::default(),
            iteration_batch: Vec::new(),
            duplicate_batch: Vec::new(),
        })
    }
}

pub(crate) struct CleanupHandlesExecutor {
    despawn_buffer: FxHashSet<Entity>,
    historical_seen: FxHashSet<u32>,
    iteration_batch: Vec<Entity>,
    duplicate_batch: Vec<Entity>,
}

impl SystemExecutor for CleanupHandlesExecutor {
    fn run(&mut self, schedule: &mut SystemsSchedule, world: &mut World) {
        world.apply_queue_commands();

        self.historical_seen.clear();
        self.iteration_batch.clear();
        self.duplicate_batch.clear();

        if !world.commands.despawns.is_empty() {
            for shard in world.commands.despawns.shards() {
                let mut lock = shard.write();
                for (entity, _) in lock.drain() {
                    let idx = entity.registry_idx();
                    self.historical_seen.insert(idx);
                    self.iteration_batch.push(entity);
                }
            }
        }

        loop {
            for entity in self.iteration_batch.drain(..) {
                self.despawn_buffer.insert(entity.clone());
                world.commands.despawns.insert(entity);
            }

            for system in schedule.systems_mut() {
                let should_run = system.run_conditions.iter().all(|cond| cond(world));
                if should_run {
                    system.run(world);
                }
            }

            world.apply_queue_commands();

            let mut found_new_unique_despawn = false;

            if !world.commands.despawns.is_empty() {
                for shard in world.commands.despawns.shards() {
                    let mut lock = shard.write();

                    for (entity, _) in lock.drain() {
                        let idx = entity.registry_idx();
                        if self.historical_seen.insert(idx) {
                            self.iteration_batch.push(entity);
                            found_new_unique_despawn = true;
                        } else {
                            self.duplicate_batch.push(entity);
                        }
                    }
                }
            }

            if found_new_unique_despawn {
                continue;
            }

            for entity in self.despawn_buffer.drain() {
                world.commands.despawns.insert(entity);
            }
            for entity in self.duplicate_batch.drain(..) {
                world.commands.despawns.insert(entity);
            }
            break;
        }

        self.historical_seen.clear();
        self.iteration_batch.clear();
        self.duplicate_batch.clear();
        world.apply_despawns();
    }
}

pub struct Last;

impl ScheduleLabel for Last {}
