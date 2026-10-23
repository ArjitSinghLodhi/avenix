use rustc_hash::FxHashSet;

use crate::{
    app::{schedule::MultiThreadedExecutor, system::SystemExt},
    entity::Entity,
    schedule::{RunConditionsList, ScheduleLabel, SystemExecutor, SystemsSchedule},
    world::{World, apply_despawns},
};

/// A schedule where queue commands are applied between each system that runs registered in this schedule.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Startup;

impl ScheduleLabel for Startup {
    fn default_executor(&mut self) -> Box<dyn SystemExecutor> {
        Box::new(StartupExecutor)
    }

    fn clone_box(&self) -> Box<dyn ScheduleLabel> {
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
                .all(|cond| cond());
            if should_run {
                system.run(world);
            }
            world.apply_queue_commands();
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct First;

impl ScheduleLabel for First {
    fn clone_box(&self) -> Box<dyn ScheduleLabel> {
        Box::new(self.clone())
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct PreUpdate;

impl ScheduleLabel for PreUpdate {
    fn clone_box(&self) -> Box<dyn ScheduleLabel> {
        Box::new(self.clone())
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Update;

impl ScheduleLabel for Update {
    fn clone_box(&self) -> Box<dyn ScheduleLabel> {
        Box::new(self.clone())
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct PostUpdate;

impl ScheduleLabel for PostUpdate {
    fn clone_box(&self) -> Box<dyn ScheduleLabel> {
        Box::new(self.clone())
    }
}

/// A special schedule where queued commands are applied immediately before
/// and after its registered systems execute every re-run.
///
/// If any system within this schedule issues a new despawn command, the entire schedule
/// re-runs. To prevent redundant processing, the engine takes out all previously handled
/// despawn commands so that only the newly issued despawns remain in the queue.
///
/// This deduplication ensures no wasted clock cycles re-processing the same entities, and
/// eliminates double-frees if entities are tied to external resources that systems in this
/// schedule manage. Ultimately, this guarantees that every cleanup system observes all pending
/// despawns before they are finalized and applied.
///
/// See all examples in `examples/despawn_cleanup` folder for guidance on how to effectively use it.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct CleanupHandles;

impl ScheduleLabel for CleanupHandles {
    fn default_executor(&mut self) -> Box<dyn SystemExecutor> {
        Box::new(CleanupHandlesExecutor::default())
    }

    fn clone_box(&self) -> Box<dyn ScheduleLabel> {
        Box::new(self.clone())
    }
}

#[derive(Default)]
pub(crate) struct CleanupHandlesExecutor {
    despawn_buffer: FxHashSet<Entity>,
    historical_seen_bits: Vec<u64>,
    iteration_batch: Vec<Entity>,
    duplicate_batch: Vec<Entity>,
    multi_threaded_executor: MultiThreadedExecutor,
}

impl SystemExecutor for CleanupHandlesExecutor {
    fn init(&mut self, schedule: &mut SystemsSchedule) {
        self.multi_threaded_executor.init(schedule);
    }
    fn run(&mut self, schedule: &mut SystemsSchedule, world: &mut World) {
        world.apply_queue_commands();

        self.historical_seen_bits.fill(0);
        self.iteration_batch.clear();
        self.duplicate_batch.clear();
        let despawns_arc = world.commands.despawns.clone();
        {
            let despawn_guard = despawns_arc.write();
            if !despawn_guard.is_empty() {
                for shard in despawn_guard.shards() {
                    let mut lock = shard.write();
                    for (entity, _) in lock.drain() {
                        let idx = entity.registry_index() as usize;
                        let word_idx = idx / 64;
                        let bit_idx = idx % 64;
                        if word_idx >= self.historical_seen_bits.len() {
                            self.historical_seen_bits.resize(word_idx + 1, 0);
                        }
                        self.historical_seen_bits[word_idx] |= 1 << bit_idx;
                        self.iteration_batch.push(entity);
                    }
                }
            }
        }

        loop {
            {
                let despawn_guard = despawns_arc.write();
                for entity in self.iteration_batch.drain(..) {
                    self.despawn_buffer.insert(entity.clone());
                    despawn_guard.insert(entity);
                }
            }

            self.multi_threaded_executor.run(schedule, world);

            world.apply_queue_commands();

            let mut despawns_guard = despawns_arc.write();

            let mut found_new_unique_despawn = false;

            if !despawns_guard.is_empty() {
                for shard in despawns_guard.shards() {
                    let mut lock = shard.write();

                    for (entity, _) in lock.drain() {
                        let idx = entity.registry_index() as usize;
                        let word_idx = idx / 64;
                        let bit_idx = idx % 64;
                        if word_idx >= self.historical_seen_bits.len() {
                            self.historical_seen_bits.resize(word_idx + 1, 0);
                        }
                        let mask = 1 << bit_idx;
                        if (self.historical_seen_bits[word_idx] & mask) == 0 {
                            self.historical_seen_bits[word_idx] |= mask;
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
            despawns_guard.extend(self.despawn_buffer.drain());
            despawns_guard.extend(self.duplicate_batch.drain(..));
            self.historical_seen_bits.fill(0);
            self.iteration_batch.clear();
            self.duplicate_batch.clear();
            apply_despawns(world, &mut despawns_guard);
            break;
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Last;

impl ScheduleLabel for Last {
    fn clone_box(&self) -> Box<dyn ScheduleLabel> {
        Box::new(self.clone())
    }
}
