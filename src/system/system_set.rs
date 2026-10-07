use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::app::schedule::DynEq;
use crate::extensions::{FunctionData, SystemExt};
use crate::schedule::{ScheduleLabel, SystemNode};
use crate::system::condition::{ConditionFn, RunConditionsList};
use crate::system::system_traits::SystemOrderings;
use crate::world::storage::World;

pub trait SystemSet: DynEq + Send + Sync + 'static {
    fn clone_box(&self) -> Box<dyn SystemSet>;
    fn create_guard_node(
        &self,
        gates: Vec<ConditionFn>,
        shared_flag: Arc<AtomicBool>,
    ) -> SystemNode;
}

impl Clone for Box<dyn SystemSet> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

#[doc(hidden)]
pub type SetConditionBuilder =
    Box<dyn FnOnce(&mut World) -> Box<dyn Fn() -> bool + Send + Sync> + Send + Sync>;

#[doc(hidden)]
pub struct GuardNode<S: SystemSet> {
    pub(crate) _marker: std::marker::PhantomData<S>,
    pub(crate) gates: Vec<ConditionFn>,
    pub(crate) shared_flag: Arc<AtomicBool>,
    pub(crate) storage: FunctionData,
}

impl<S: SystemSet> crate::system::SystemData for GuardNode<S> {
    fn get_raw(&self, id: std::any::TypeId) -> Option<&Box<dyn std::any::Any + Send + Sync>> {
        self.storage.get_raw_data(&id)
    }

    fn get_raw_mut(
        &mut self,
        id: std::any::TypeId,
    ) -> Option<&mut Box<dyn std::any::Any + Send + Sync>> {
        self.storage.get_raw_data_mut(&id)
    }

    fn insert_raw(&mut self, id: std::any::TypeId, value: Box<dyn std::any::Any + Send + Sync>) {
        self.storage.insert_raw_data(&id, value)
    }
}

impl<S: SystemSet> crate::system::System for GuardNode<S> {
    fn run(&mut self, _world: &World) {
        let result = self.gates.iter().all(|cond| cond());
        self.shared_flag
            .store(result, std::sync::atomic::Ordering::Release);
    }

    fn func_type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
    }

    fn name(&self) -> &'static str {
        "Avenix::GuardNode"
    }
}

impl<S: SystemSet> GuardNode<S> {
    #[doc(hidden)]
    pub fn new(gates: Vec<ConditionFn>, shared_flag: Arc<AtomicBool>) -> Self {
        Self {
            _marker: PhantomData,
            gates,
            shared_flag,
            storage: FunctionData::new(),
        }
    }
}

pub(crate) struct SetRegistration {
    pub(crate) schedule: Box<dyn ScheduleLabel>,
    pub(crate) set: Box<dyn SystemSet>,
    pub(crate) run_after_sets: Vec<Box<dyn SystemSet>>,
    pub(crate) run_before_sets: Vec<Box<dyn SystemSet>>,
    pub(crate) run_after_systems: Vec<std::any::TypeId>,
    pub(crate) run_before_systems: Vec<std::any::TypeId>,
    pub(crate) member_of: Vec<Box<dyn SystemSet>>,
    pub(crate) condition_builders: Vec<SetConditionBuilder>,
}
pub(crate) fn preprocess_system_sets(
    schedule_systems: &mut [&mut SystemNode],
    registrations: &mut [SetRegistration],
    world: &mut World,
    generated_guards: &mut Vec<SystemNode>,
) {
    let n_reg = registrations.len();
    let mut nesting_matrix = vec![vec![false; n_reg]; n_reg];
    for i in 0..n_reg {
        for parent in &registrations[i].member_of {
            if let Some(p_idx) = registrations
                .iter()
                .position(|r| r.set.dyn_eq(parent.as_ref()))
            {
                nesting_matrix[i][p_idx] = true;
            }
        }
    }
    for k in 0..n_reg {
        for i in 0..n_reg {
            for j in 0..n_reg {
                if nesting_matrix[i][k] && nesting_matrix[k][j] {
                    nesting_matrix[i][j] = true;
                }
            }
        }
    }
    #[allow(clippy::needless_range_loop)]
    for i in 0..n_reg {
        if nesting_matrix[i][i] {
            panic!(
                "❌ SCHEDULING ERROR: A cyclic nesting deadlock was detected! A SystemSet cannot be configured to contain itself hierarchically."
            );
        }
    }

    let mut structural_changes = true;
    while structural_changes {
        structural_changes = false;
        for set_reg in registrations.iter_mut() {
            let child_set = set_reg.set.clone();
            let parent_sets = set_reg.member_of.clone();

            if !parent_sets.is_empty() {
                for node in schedule_systems.iter_mut() {
                    let sys = node.system_mut();
                    let orderings =
                        sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);

                    if orderings
                        .member_of
                        .iter()
                        .any(|s| s.dyn_eq(child_set.as_ref()))
                    {
                        for parent in &parent_sets {
                            if !orderings
                                .member_of
                                .iter()
                                .any(|s| s.dyn_eq(parent.as_ref()))
                            {
                                orderings.member_of.push(parent.clone());
                                structural_changes = true;
                            }
                        }
                    }
                }
            }
        }
    }

    for node in schedule_systems.iter_mut() {
        let sys = node.system_mut();
        let orderings = sys.get_or_init::<SystemOrderings>(SystemOrderings::default);
        for member_set in &orderings.member_of {
            let is_registered = registrations
                .iter()
                .any(|reg| reg.set.dyn_eq(member_set.as_ref()));

            if !is_registered {
                panic!(
                    "❌ SCHEDULING ERROR: System '{}' is configured to run inside a SystemSet which was never registered via app.configure_set()!",
                    sys.name()
                );
            }
        }
    }

    let mut guard_nodes: Vec<(Box<dyn SystemSet>, Arc<AtomicBool>, SystemNode)> =
        Vec::with_capacity(registrations.len());

    for reg in registrations.iter_mut() {
        if reg.condition_builders.is_empty() {
            continue;
        }

        let mut gates = Vec::with_capacity(reg.condition_builders.len());
        for builder in reg.condition_builders.drain(..) {
            gates.push(builder(world));
        }

        let shared_flag = Arc::new(AtomicBool::new(false));
        let set_identity = reg.set.clone();
        let guard_node = reg.set.create_guard_node(gates, shared_flag.clone());

        guard_nodes.push((set_identity, shared_flag, guard_node));
    }

    for node in schedule_systems.iter_mut() {
        let sys = node.system_mut();
        let orderings = sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);

        let mut matching_flags = Vec::new();
        for member_set in &orderings.member_of {
            for (guard_set, shared_flag, guard_node) in &guard_nodes {
                if member_set.dyn_eq(guard_set.as_ref()) {
                    matching_flags.push(shared_flag.clone());

                    let guard_type_id = guard_node.system().func_type_id();
                    orderings.run_after_systems.push(guard_type_id);
                }
            }
        }

        if !matching_flags.is_empty() {
            let list = sys.get_or_init_mut::<RunConditionsList>(RunConditionsList::default);
            for flag in matching_flags {
                let gate: ConditionFn = Box::new(move || flag.load(Ordering::Relaxed));
                list.conditions_mut().push(gate);
            }
        }
    }

    for reg in registrations.iter() {
        let mut target_before_type_ids = reg.run_before_systems.clone();
        for target_set in &reg.run_before_sets {
            for node in schedule_systems.iter_mut() {
                let sys = node.system_mut();
                let orderings = sys.get_or_init::<SystemOrderings>(SystemOrderings::default);
                if orderings
                    .member_of
                    .iter()
                    .any(|s| s.dyn_eq(target_set.as_ref()))
                {
                    target_before_type_ids.push(sys.func_type_id());
                }
            }
        }

        let mut target_after_type_ids = reg.run_after_systems.clone();
        for target_set in &reg.run_after_sets {
            for node in schedule_systems.iter_mut() {
                let sys = node.system_mut();
                let orderings = sys.get_or_init::<SystemOrderings>(SystemOrderings::default);
                if orderings
                    .member_of
                    .iter()
                    .any(|s| s.dyn_eq(target_set.as_ref()))
                {
                    target_after_type_ids.push(sys.func_type_id());
                }
            }
        }

        for node in schedule_systems.iter_mut() {
            let sys = node.system_mut();
            let orderings = sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);
            let is_member = orderings
                .member_of
                .iter()
                .any(|s| s.dyn_eq(reg.set.as_ref()));

            if is_member {
                orderings
                    .run_before_systems
                    .extend(target_before_type_ids.clone());
                orderings
                    .run_after_systems
                    .extend(target_after_type_ids.clone());
            }
        }
    }

    struct SystemSetMemberCache {
        type_id: std::any::TypeId,
        member_of: Vec<Box<dyn SystemSet>>,
    }

    let member_cache: Vec<SystemSetMemberCache> = schedule_systems
        .iter_mut()
        .map(|node| {
            let sys = node.system_mut();
            let tid = sys.func_type_id();
            let orderings = sys.get_or_init::<SystemOrderings>(SystemOrderings::default);
            SystemSetMemberCache {
                type_id: tid,
                member_of: orderings.member_of.iter().map(|s| s.clone_box()).collect(),
            }
        })
        .collect();

    for node in schedule_systems.iter_mut() {
        let sys = node.system_mut();
        let orderings = sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);

        let local_before_sets = std::mem::take(&mut orderings.run_before_sets);
        for target_set in local_before_sets {
            for cache_item in &member_cache {
                if cache_item
                    .member_of
                    .iter()
                    .any(|s| s.dyn_eq(target_set.as_ref()))
                {
                    orderings.run_before_systems.push(cache_item.type_id);
                }
            }
        }

        let local_after_sets = std::mem::take(&mut orderings.run_after_sets);
        for target_set in local_after_sets {
            for cache_item in &member_cache {
                if cache_item
                    .member_of
                    .iter()
                    .any(|s| s.dyn_eq(target_set.as_ref()))
                {
                    orderings.run_after_systems.push(cache_item.type_id);
                }
            }
        }
    }

    for (_, _, guard_node) in guard_nodes {
        generated_guards.push(guard_node);
    }
}
