use std::collections::VecDeque;

use crate::schedule::{Schedule, ScheduleConstraint, Startup};

const SCHEDULE_ERROR_STR: &str = "SchedulesSortingError";

pub(crate) fn sort_schedules(
    unarranged_schedules: Vec<Schedule>,
    schedule_order_constraints: &Vec<ScheduleConstraint>,
) -> Vec<Schedule> {
    let unarranged = unarranged_schedules;

    let mut unique_schedules: Vec<Schedule> = Vec::with_capacity(unarranged.len());
    for s in unarranged {
        if unique_schedules
            .iter()
            .any(|existing| (*existing.label()).dyn_eq(s.label()))
        {
            panic!("{}\nDuplicate Schedule detected!", SCHEDULE_ERROR_STR);
        }
        unique_schedules.push(s);
    }

    let total_schedules = unique_schedules.len();
    let mut adjacency_list: Vec<Vec<usize>> = vec![Vec::new(); total_schedules];
    let mut in_degree: Vec<usize> = vec![0; total_schedules];

    for ScheduleConstraint { before, after } in schedule_order_constraints {
        if before.dyn_eq(&Startup) || after.dyn_eq(&Startup) {
            panic!(
                "{}\nOrdering constraint references the 'Startup' root! Startup is completely isolated from dynamic ordering rules.",
                SCHEDULE_ERROR_STR
            );
        }
        let before_idx = unique_schedules
            .iter()
            .position(|s| (*s.label()).dyn_eq(&**before))
            .or_else(|| {
                panic!(
                    "{}\nOrdering constraint references an unregistered schedule: '{:?}'",
                    SCHEDULE_ERROR_STR, before
                );
            });

        let after_idx = unique_schedules
            .iter()
            .position(|s| (*s.label()).dyn_eq(&**after))
            .or_else(|| {
                panic!(
                    "{}\nOrdering constraint references an unregistered schedule: '{:?}'",
                    SCHEDULE_ERROR_STR, after
                );
            });

        let b_idx = before_idx.unwrap();
        let a_idx = after_idx.unwrap();

        adjacency_list[b_idx].push(a_idx);
        in_degree[a_idx] += 1;
    }

    let mut sorted_indices = Vec::with_capacity(total_schedules);
    let mut queue: VecDeque<usize> = in_degree
        .iter()
        .enumerate()
        .filter(|&(_, &deg)| deg == 0)
        .map(|(idx, _)| idx)
        .collect();

    while let Some(u) = queue.pop_front() {
        sorted_indices.push(u);

        for &v in &adjacency_list[u] {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push_back(v);
            }
        }
    }

    if sorted_indices.len() != total_schedules {
        let mut trapped_schedules = Vec::new();
        for (idx, s) in unique_schedules.iter().enumerate() {
            if !sorted_indices.contains(&idx) {
                trapped_schedules.push(format!("  • {:?}", s.label()));
            }
        }
        panic!(
            "{}\nCircular dependency deadlock detected in Schedule constraints!\n\nThe following schedules are deadlocked in the cycle:\n{}\n",
            SCHEDULE_ERROR_STR,
            trapped_schedules.join("\n")
        );
    }

    let mut sorted_schedules = Vec::with_capacity(total_schedules);
    let mut temp_movable: Vec<Option<Schedule>> = unique_schedules.into_iter().map(Some).collect();

    for idx in sorted_indices {
        sorted_schedules.push(temp_movable[idx].take().unwrap());
    }

    sorted_schedules
}
