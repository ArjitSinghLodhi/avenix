use rustc_hash::FxHashSet;

use crate::{
    entity::Entity,
    extensions::SystemExt,
    schedule::{RunConditionsList, ScheduleLabel, SystemExecutor, SystemsSchedule},
    world::storage::{World, apply_despawns},
};

/// A schedule where queue commands are applied between each system that runs registered in this schedule.
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
            let should_run = system
                .system
                .get_or_init(RunConditionsList::default)
                .run_conditions
                .iter()
                .all(|cond| cond(world));
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
/// see the hierarchy_cleanup example on how to effectively use it.
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
        let despawns_arc = world.commands.despawns.clone();
        {
            let despawn_gaurd = despawns_arc.write();
            if !despawn_gaurd.is_empty() {
                for shard in despawn_gaurd.shards() {
                    let mut lock = shard.write();
                    for (entity, _) in lock.drain() {
                        let idx = entity.registry_idx();
                        self.historical_seen.insert(idx);
                        self.iteration_batch.push(entity);
                    }
                }
            }
        }

        loop {
            {
                let despawn_gaurd = despawns_arc.write();
                for entity in self.iteration_batch.drain(..) {
                    self.despawn_buffer.insert(entity.clone());
                    despawn_gaurd.insert(entity);
                }
            }

            for system in schedule.systems_mut() {
                let should_run = system
                    .system
                    .get_or_init(RunConditionsList::default)
                    .run_conditions
                    .iter()
                    .all(|cond| cond(world));
                if should_run {
                    system.run(world);
                }
            }

            world.apply_queue_commands();

            let mut despawns_gaurd = despawns_arc.write();

            let mut found_new_unique_despawn = false;

            if !despawns_gaurd.is_empty() {
                for shard in despawns_gaurd.shards() {
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
            despawns_gaurd.extend(self.despawn_buffer.drain());
            despawns_gaurd.extend(self.duplicate_batch.drain(..));
            self.historical_seen.clear();
            self.iteration_batch.clear();
            self.duplicate_batch.clear();
            apply_despawns(world, &mut despawns_gaurd);
            break;
        }
    }
}

pub struct Last;

impl ScheduleLabel for Last {}
