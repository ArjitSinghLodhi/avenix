use std::any::TypeId;

use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::{
    app_impl::SystemsBlock,
    extensions::SystemExt,
    schedule::{CleanupHandles, Schedule, Startup, SystemNode},
    system::{SystemMeta, system_traits::SystemOrderings},
};

pub struct SystemAccessView<'a> {
    pub component_reads: &'a [TypeId],
    pub component_writes: &'a [TypeId],
    pub resource_reads: &'a [TypeId],
    pub resource_writes: &'a [TypeId],
    pub with_filters: &'a [TypeId],
    pub without_filters: &'a [TypeId],
}

pub fn check_access_conflicts(a: &SystemAccessView, b: &SystemAccessView) -> bool {
    for r in a.resource_reads {
        if b.resource_writes.contains(r) {
            return true;
        }
    }

    for w in a.resource_writes {
        if b.resource_writes.contains(w) || b.resource_reads.contains(w) {
            return true;
        }
    }

    for r in a.component_reads {
        if b.component_writes.contains(r) {
            let is_disjoint = a.with_filters.iter().any(|f| b.without_filters.contains(f))
                || a.without_filters.iter().any(|f| b.with_filters.contains(f));
            if !is_disjoint {
                return true;
            }
        }
    }

    for w in a.component_writes {
        if b.component_writes.contains(w) || b.component_reads.contains(w) {
            let is_disjoint = a.with_filters.iter().any(|f| b.without_filters.contains(f))
                || a.without_filters.iter().any(|f| b.with_filters.contains(f));
            if !is_disjoint {
                return true;
            }
        }
    }

    false
}

pub(crate) fn dispatch_system_blocks(
    systems_blocks: &mut Vec<SystemsBlock>,
    startup_schedule: &mut Schedule,
    cleanup_schedule: &mut Schedule,
    schedules: &mut [Schedule],
) {
    for system_block in systems_blocks.drain(..) {
        if (*system_block.schedule).dyn_eq(&Startup) {
            for system in system_block.systems {
                startup_schedule.add_system_boxed(system);
            }
            continue;
        }
        if (*system_block.schedule).dyn_eq(&CleanupHandles) {
            for system in system_block.systems {
                cleanup_schedule.add_system_boxed(system);
            }
            continue;
        }

        let target_schedule = match schedules
            .iter_mut()
            .find(|s| (*s.schedule()).dyn_eq(&*system_block.schedule))
        {
            Some(schedule) => schedule,
            None => {
                panic!(
                    "❌ CONFIGURATION ERROR: Attempted to add systems to Schedule '{:?}' which was never registered via add_schedule()!",
                    system_block.schedule
                );
            }
        };

        for system in system_block.systems {
            target_schedule.add_system_boxed(system);
        }
    }
}

pub(crate) fn sort_schedule_systems(schedule_systems: &mut Vec<SystemNode>) {
    struct SortingMeta {
        type_id: TypeId,
        name: &'static str,
        run_after_systems: Vec<TypeId>,
        run_before_systems: Vec<TypeId>,
        is_send: bool,
        component_reads: Vec<TypeId>,
        component_writes: Vec<TypeId>,
        resource_reads: Vec<TypeId>,
        resource_writes: Vec<TypeId>,
        with_filters: Vec<TypeId>,
        without_filters: Vec<TypeId>,
    }

    let n = schedule_systems.len();
    let mut meta_list = Vec::with_capacity(n);
    for node in schedule_systems.iter_mut() {
        let system_mut_ref = node.system_mut();
        let tid = system_mut_ref.func_type_id();
        let sys_name = system_mut_ref.name();

        let meta = system_mut_ref.get_or_init(SystemMeta::default);
        let is_send = meta.is_send();
        let component_reads = meta.component_reads().copied().collect();
        let component_writes = meta.component_writes().copied().collect();
        let resource_reads = meta.resource_reads().copied().collect();
        let resource_writes = meta.resource_writes().copied().collect();
        let with_filters = meta.with_filters().copied().collect();
        let without_filters = meta.without_filters().copied().collect();

        let orderings = system_mut_ref.get_or_init(SystemOrderings::default);
        let run_after_systems = orderings.run_after_systems.clone();
        let run_before_systems = orderings.run_before_systems.clone();

        meta_list.push(SortingMeta {
            type_id: tid,
            name: sys_name,
            run_after_systems,
            run_before_systems,
            is_send,
            component_reads,
            component_writes,
            resource_reads,
            resource_writes,
            with_filters,
            without_filters,
        });
    }

    let mut type_to_idx: FxHashMap<TypeId, usize> =
        FxHashMap::with_capacity_and_hasher(n, FxBuildHasher);
    for (idx, meta) in meta_list.iter().enumerate() {
        type_to_idx.insert(meta.type_id, idx);
    }

    let mut graph: Vec<Vec<usize>> = vec![vec![]; n];
    let mut in_degree = vec![0; n];

    for (idx, meta) in meta_list.iter().enumerate() {
        for target_type in &meta.run_after_systems {
            if *target_type == meta.type_id {
                panic!(
                    "❌ SCHEDULING ERROR: System '{}' cannot be configured to run after itself!",
                    meta.name
                );
            }
            match type_to_idx.get(target_type) {
                Some(&target_idx) => {
                    graph[target_idx].push(idx);
                    in_degree[idx] += 1;
                }
                None => {
                    panic!(
                        "❌ SCHEDULING ERROR: System '{}' attempted to schedule a dependency targeting a system that does not exist in this schedule!",
                        meta.name
                    );
                }
            }
        }
        for target_type in &meta.run_before_systems {
            if *target_type == meta.type_id {
                panic!(
                    "❌ SCHEDULING ERROR: System '{}' cannot be configured to run before itself!",
                    meta.name
                );
            }
            match type_to_idx.get(target_type) {
                Some(&target_idx) => {
                    graph[idx].push(target_idx);
                    in_degree[target_idx] += 1;
                }
                None => {
                    panic!(
                        "❌ SCHEDULING ERROR: System '{}' attempted to schedule a dependency targeting a system that does not exist in this schedule!",
                        meta.name
                    );
                }
            }
        }
    }

    let mut sorted_indices = Vec::with_capacity(n);
    let mut completed = vec![false; n];

    let mut active_batch_systems: Vec<usize> = Vec::new();

    while sorted_indices.len() < n {
        let mut candidates = Vec::new();
        for i in 0..n {
            if !completed[i] && in_degree[i] == 0 {
                candidates.push(i);
            }
        }

        if candidates.is_empty() {
            let mut stuck_systems = Vec::new();
            for i in 0..n {
                if !completed[i] {
                    stuck_systems.push(format!("  • {}", meta_list[i].name));
                }
            }
            panic!(
                "❌ SCHEDULING ERROR: A circular dependency cycle was detected between systems!\n\nThe following systems are deadlocked in the cycle:\n{}\n",
                stuck_systems.join("\n")
            );
        }

        let mut batch_indices = Vec::new();
        active_batch_systems.clear();
        let mut has_non_send = false;

        for &idx in &candidates {
            let meta = &meta_list[idx];

            if !meta.is_send {
                if batch_indices.is_empty() {
                    batch_indices.push(idx);
                    break;
                } else {
                    continue;
                }
            }

            if has_non_send {
                continue;
            }

            let view_a = SystemAccessView {
                component_reads: &meta.component_reads,
                component_writes: &meta.component_writes,
                resource_reads: &meta.resource_reads,
                resource_writes: &meta.resource_writes,
                with_filters: &meta.with_filters,
                without_filters: &meta.without_filters,
            };

            let mut conflict = false;

            for &active_idx in &active_batch_systems {
                let active_meta = &meta_list[active_idx];

                let view_b = SystemAccessView {
                    component_reads: &active_meta.component_reads,
                    component_writes: &active_meta.component_writes,
                    resource_reads: &active_meta.resource_reads,
                    resource_writes: &active_meta.resource_writes,
                    with_filters: &active_meta.with_filters,
                    without_filters: &active_meta.without_filters,
                };

                if check_access_conflicts(&view_a, &view_b) {
                    conflict = true;
                    break;
                }
            }

            if conflict {
                continue;
            }

            if !meta.is_send {
                has_non_send = true;
            }

            batch_indices.push(idx);
            active_batch_systems.push(idx);
        }

        for &idx in &batch_indices {
            completed[idx] = true;
            sorted_indices.push(idx);
            for &v in &graph[idx] {
                in_degree[v] -= 1;
            }
        }
    }

    let old_systems = std::mem::take(schedule_systems);
    let mut temp_map: FxHashMap<usize, SystemNode> = old_systems.into_iter().enumerate().collect();

    for idx in sorted_indices {
        if let Some(node) = temp_map.remove(&idx) {
            schedule_systems.push(node);
        }
    }
}
