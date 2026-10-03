use std::{any::TypeId, collections::VecDeque};

use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::{
    app_impl::SystemsBlock,
    extensions::SystemExt,
    schedule::{CleanupHandles, IntoScheduleId, Schedule, Startup, SystemNode},
    system::system_traits::SystemOrderings,
};

pub(crate) fn dispatch_system_blocks(
    systems_blocks: &mut Vec<SystemsBlock>,
    startup_schedule: &mut Schedule,
    cleanup_schedule: &mut Schedule,
    schedules: &mut [Schedule],
) {
    for system_block in systems_blocks.drain(..) {
        if system_block.schedule_id == Startup.id() {
            for system in system_block.systems {
                startup_schedule.add_system_boxed(system);
            }
            continue;
        }
        if system_block.schedule_id == CleanupHandles.id() {
            for system in system_block.systems {
                cleanup_schedule.add_system_boxed(system);
            }
            continue;
        }

        let target_schedule = match schedules
            .iter_mut()
            .find(|s| s.id() == system_block.schedule_id)
        {
            Some(schedule) => schedule,
            None => {
                let missing_name = system_block.schedule_id.name;
                panic!(
                    "❌ CONFIGURATION ERROR: Attempted to add systems to Schedule '{}' which was never registered via add_schedule()!",
                    missing_name
                );
            }
        };

        for system in system_block.systems {
            target_schedule.add_system_boxed(system);
        }
    }
}

pub(crate) fn sort_schedule_systems(schedule_systems: &mut Vec<SystemNode>) {
    let n = schedule_systems.len();
    if n <= 1 {
        return;
    }

    struct SortingMeta {
        type_id: TypeId,
        name: &'static str,
        run_after: Vec<TypeId>,
        run_before: Vec<TypeId>,
    }

    let mut meta_list = Vec::with_capacity(n);
    for node in schedule_systems.iter_mut().take(n) {
        unsafe {
            let node_ptr = node as *mut SystemNode;
            let system_ref = (*node_ptr).system();
            let tid = system_ref.pub_type_id();
            let sys_name = system_ref.name();

            let system_mut_ref = (*node_ptr).system_mut();
            let orderings = system_mut_ref.get_or_init(SystemOrderings::default);

            meta_list.push(SortingMeta {
                type_id: tid,
                name: sys_name,
                run_after: orderings.run_after.clone(),
                run_before: orderings.run_before.clone(),
            });
        }
    }

    let mut type_to_idx: FxHashMap<TypeId, usize> =
        FxHashMap::with_capacity_and_hasher(n, FxBuildHasher);
    for (idx, meta) in meta_list.iter().enumerate() {
        type_to_idx.insert(meta.type_id, idx);
    }

    let mut graph: Vec<Vec<usize>> = vec![vec![]; n];
    let mut in_degree = vec![0; n];

    for (idx, meta) in meta_list.iter().enumerate() {
        for target_type in &meta.run_after {
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
        for target_type in &meta.run_before {
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

    let mut queue = VecDeque::new();
    for (i, &deg) in in_degree.iter().enumerate().take(n) {
        if deg == 0 {
            queue.push_back(i);
        }
    }

    let mut sorted_indices = Vec::with_capacity(n);
    while let Some(u) = queue.pop_front() {
        sorted_indices.push(u);
        for &v in &graph[u] {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push_back(v);
            }
        }
    }

    if sorted_indices.len() != n {
        let mut stuck_systems = Vec::new();
        for i in 0..n {
            if in_degree[i] > 0 {
                stuck_systems.push(format!("  • {}", meta_list[i].name));
            }
        }
        panic!(
            "❌ SCHEDULING ERROR: A circular dependency cycle was detected between systems!\n\nThe following systems are deadlocked in the cycle:\n{}\n",
            stuck_systems.join("\n")
        );
    }

    let old_systems = std::mem::take(schedule_systems);
    let mut temp_map: FxHashMap<usize, SystemNode> = old_systems.into_iter().enumerate().collect();

    for idx in sorted_indices {
        if let Some(node) = temp_map.remove(&idx) {
            schedule_systems.push(node);
        }
    }
}
