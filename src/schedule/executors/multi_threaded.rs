use std::any::TypeId;

use crate::{
    extensions::{SystemExt, World},
    schedule::{SystemExecutor, SystemsSchedule},
    system::{SystemMeta, condition::RunConditionsList, system_traits::SystemOrderings},
};

pub struct MultiThreadedExecutor {
    parallel_batches: Vec<ParallelSystemsBatchRange>,
}

impl Default for MultiThreadedExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiThreadedExecutor {
    pub fn new() -> Self {
        Self {
            parallel_batches: Vec::new(),
        }
    }
}

struct ParallelSystemsBatchRange {
    start: usize,
    end: usize,
    executio_mode: ExecutionMode,
}

enum ExecutionMode {
    Sequential,
    Parallel,
}

impl SystemExecutor for MultiThreadedExecutor {
    fn init(&mut self, schedule: &mut SystemsSchedule) {
        self.parallel_batches.clear();
        let systems = schedule.systems_mut();
        let n = systems.len();
        if n == 0 {
            return;
        }

        struct CachedMeta {
            is_send: bool,
            type_id: TypeId,
            run_after: Vec<TypeId>,
            run_before: Vec<TypeId>,
            component_reads: Vec<TypeId>,
            component_writes: Vec<TypeId>,
            resource_reads: Vec<TypeId>,
            resource_writes: Vec<TypeId>,
            with_filters: Vec<TypeId>,
            without_filters: Vec<TypeId>,
        }

        let mut cached_meta_list = Vec::with_capacity(n);
        for sys_node in systems {
            let system_mut_ref = sys_node.system_mut();
            let orderings = system_mut_ref.get_or_init(SystemOrderings::default).clone();
            let func_id = system_mut_ref.func_type_id();
            let meta = system_mut_ref.get_or_init(SystemMeta::default);

            cached_meta_list.push(CachedMeta {
                is_send: meta.is_send(),
                type_id: func_id,
                run_after: orderings.run_after.clone(),
                run_before: orderings.run_before.clone(),
                component_reads: meta.component_reads().copied().collect(),
                component_writes: meta.component_writes().copied().collect(),
                resource_reads: meta.resource_reads().copied().collect(),
                resource_writes: meta.resource_writes().copied().collect(),
                with_filters: meta.with_filters().copied().collect(),
                without_filters: meta.without_filters().copied().collect(),
            });
        }

        let mut current_start = 0;
        let mut i = 0;

        while i < n {
            let meta = &cached_meta_list[i];

            if !meta.is_send {
                if i > current_start {
                    let len = i - current_start;
                    self.parallel_batches.push(ParallelSystemsBatchRange {
                        start: current_start,
                        end: i,
                        executio_mode: if len > 1 {
                            ExecutionMode::Parallel
                        } else {
                            ExecutionMode::Sequential
                        },
                    });
                }
                self.parallel_batches.push(ParallelSystemsBatchRange {
                    start: i,
                    end: i + 1,
                    executio_mode: ExecutionMode::Sequential,
                });
                current_start = i + 1;
                i += 1;
                continue;
            }

            let mut conflict = false;

            #[allow(clippy::needless_range_loop)]
            for active_idx in current_start..i {
                let active_meta = &cached_meta_list[active_idx];

                if meta.run_after.contains(&active_meta.type_id) {
                    conflict = true;
                    break;
                }

                if meta.run_before.contains(&active_meta.type_id) {
                    conflict = true;
                    break;
                }

                if active_meta.run_before.contains(&meta.type_id) {
                    conflict = true;
                    break;
                }
                if active_meta.run_after.contains(&meta.type_id) {
                    conflict = true;
                    break;
                }
                for r in &meta.resource_reads {
                    if active_meta.resource_writes.contains(r) {
                        conflict = true;
                        break;
                    }
                }
                if conflict {
                    break;
                }

                for w in &meta.resource_writes {
                    if active_meta.resource_writes.contains(w)
                        || active_meta.resource_reads.contains(w)
                    {
                        conflict = true;
                        break;
                    }
                }
                if conflict {
                    break;
                }

                for r in &meta.component_reads {
                    if active_meta.component_writes.contains(r) {
                        let is_disjoint = meta
                            .with_filters
                            .iter()
                            .any(|f| active_meta.without_filters.contains(f))
                            || meta
                                .without_filters
                                .iter()
                                .any(|f| active_meta.with_filters.contains(f));
                        if !is_disjoint {
                            conflict = true;
                            break;
                        }
                    }
                }
                if conflict {
                    break;
                }

                for w in &meta.component_writes {
                    if active_meta.component_writes.contains(w)
                        || active_meta.component_reads.contains(w)
                    {
                        let is_disjoint = meta
                            .with_filters
                            .iter()
                            .any(|f| active_meta.without_filters.contains(f))
                            || meta
                                .without_filters
                                .iter()
                                .any(|f| active_meta.with_filters.contains(f));
                        if !is_disjoint {
                            conflict = true;
                            break;
                        }
                    }
                }
                if conflict {
                    break;
                }
            }

            if conflict {
                let len = i - current_start;
                self.parallel_batches.push(ParallelSystemsBatchRange {
                    start: current_start,
                    end: i,
                    executio_mode: if len > 1 {
                        ExecutionMode::Parallel
                    } else {
                        ExecutionMode::Sequential
                    },
                });
                current_start = i;
            }

            i += 1;
        }

        if current_start < n {
            let len = n - current_start;
            self.parallel_batches.push(ParallelSystemsBatchRange {
                start: current_start,
                end: n,
                executio_mode: if len > 1 {
                    ExecutionMode::Parallel
                } else {
                    ExecutionMode::Sequential
                },
            });
        }
    }

    fn run(&mut self, schedule: &mut SystemsSchedule, world: &mut World) {
        let systems = schedule.systems_mut();
        for batch in &self.parallel_batches {
            let slice = &mut systems[batch.start..batch.end];

            match batch.executio_mode {
                ExecutionMode::Parallel => {
                    use rayon::prelude::*;
                    slice.par_iter_mut().for_each(|node| {
                        let should_run = node
                            .system_mut()
                            .get_or_init(RunConditionsList::default)
                            .conditions()
                            .iter()
                            .all(|cond| cond());

                        if should_run {
                            node.run(world);
                        }
                    });
                }
                ExecutionMode::Sequential => {
                    for node in slice {
                        let should_run = node
                            .system_mut()
                            .get_or_init(RunConditionsList::default)
                            .conditions()
                            .iter()
                            .all(|cond| cond());

                        if should_run {
                            node.run(world);
                        }
                    }
                }
            }
        }
    }
}
