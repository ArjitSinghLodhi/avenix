use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::app::schedule::DynEq;
use crate::app::system::{SystemData, SystemExt};
use crate::schedule::{ScheduleLabel, SystemNode};
use crate::system::condition::{ConditionFn, RunConditionsList};
use crate::system::system_storage::FunctionData;
use crate::system::system_traits::SystemOrderings;
use crate::system::{System, SystemIdentity};
use crate::world::World;
use std::fmt::Debug;

pub trait SystemSet: Debug + DynEq + Send + Sync + 'static {
    #[doc(hidden)]
    fn dyn_clone(&self) -> Box<dyn SystemSet>;
    #[doc(hidden)]
    fn create_guard_node(
        &self,
        gates: Vec<ConditionFn>,
        shared_flag: Arc<AtomicBool>,
    ) -> SystemNode;
}

#[doc(hidden)]
pub struct GuardNode<S: SystemSet> {
    pub(crate) _marker: PhantomData<S>,
    pub(crate) gates: Vec<ConditionFn>,
    pub(crate) shared_flag: Arc<AtomicBool>,
    pub(crate) storage: FunctionData,
    pub(crate) identity: GuardIdentity,
}

impl<S: SystemSet> SystemData for GuardNode<S> {
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

    fn remove_raw(&mut self, id: std::any::TypeId) -> Option<Box<dyn Any + Send + Sync + 'static>> {
        self.storage.remove_raw(id)
    }
}

impl<S: SystemSet> System for GuardNode<S> {
    fn run(&mut self, world: &World) {
        let result = self.gates.iter().all(|cond| cond(world));
        self.shared_flag.store(result, Ordering::Release);
    }

    fn identity(&self) -> &dyn SystemIdentity {
        &self.identity
    }
}

impl<S: SystemSet> GuardNode<S> {
    #[doc(hidden)]
    pub fn new(
        set: Box<dyn SystemSet>,
        gates: Vec<ConditionFn>,
        shared_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            _marker: PhantomData,
            gates,
            shared_flag,
            storage: FunctionData::new(),
            identity: GuardIdentity {
                set: set.dyn_clone(),
                name: format!("GuardNodeSet: {:?}", set),
            },
        }
    }
}

pub struct GuardIdentity {
    set: Box<dyn SystemSet>,
    name: String,
}

impl GuardIdentity {
    pub fn system_set(&self) -> &dyn SystemSet {
        &*self.set
    }
}

impl SystemIdentity for GuardIdentity {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn dyn_system_eq(&self, other: &dyn SystemIdentity) -> bool {
        if let Some(other_concrete) = other.as_any().downcast_ref::<Self>() {
            self.set.dyn_eq(other_concrete.set.as_ref())
        } else {
            false
        }
    }
    fn dyn_clone(&self) -> Box<dyn SystemIdentity> {
        Box::new(GuardIdentity {
            set: self.set.dyn_clone(),
            name: self.name.clone(),
        })
    }
    fn display_name(&self) -> &str {
        &self.name
    }
}

pub(crate) struct SetRegistration {
    pub(crate) schedule: Box<dyn ScheduleLabel>,
    pub(crate) set: Box<dyn SystemSet>,
    pub(crate) run_after_sets: Vec<Box<dyn SystemSet>>,
    pub(crate) run_before_sets: Vec<Box<dyn SystemSet>>,
    pub(crate) run_after_systems: Vec<Box<dyn SystemIdentity>>,
    pub(crate) run_before_systems: Vec<Box<dyn SystemIdentity>>,
    pub(crate) member_of: Vec<Box<dyn SystemSet>>,
    pub(crate) condition_gates: Vec<ConditionFn>,
}

pub(crate) fn preprocess_system_sets(
    schedule_systems: &mut [&mut SystemNode],
    registrations: &mut [SetRegistration],
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
                "❌ SystemSet Error: A cyclic nesting deadlock was detected! A SystemSet cannot be configured to contain itself hierarchically."
            );
        }
    }

    for reg in registrations.iter() {
        for target_set in &reg.run_before_sets {
            if !registrations
                .iter()
                .any(|r| r.set.dyn_eq(target_set.as_ref()))
            {
                panic!(
                    "❌ SystemSet Error: A SystemSet was configured to run before a set that was never registered via app.configure_set()!"
                );
            }
        }
        for target_set in &reg.run_after_sets {
            if !registrations
                .iter()
                .any(|r| r.set.dyn_eq(target_set.as_ref()))
            {
                panic!(
                    "❌ SystemSet Error: A SystemSet was configured to run after a set that was never registered via app.configure_set()!"
                );
            }
        }
    }

    let mut structural_changes = true;
    while structural_changes {
        structural_changes = false;
        for set_reg in registrations.iter_mut() {
            let child_set = set_reg.set.dyn_clone();
            let parent_sets = set_reg
                .member_of
                .iter()
                .map(|set| set.dyn_clone())
                .collect::<Vec<_>>();

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
                                orderings.member_of.push(parent.dyn_clone());
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
        let orderings = sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);
        for member_set in &orderings.member_of {
            let is_registered = registrations
                .iter()
                .any(|reg| reg.set.dyn_eq(member_set.as_ref()));

            if !is_registered {
                panic!(
                    "❌ SystemSet Error: System '{}' is configured to run inside a SystemSet which was never registered via app.configure_set()!",
                    sys.identity().display_name()
                );
            }
        }
    }

    let mut guard_nodes: Vec<(Box<dyn SystemSet>, Arc<AtomicBool>, SystemNode)> =
        Vec::with_capacity(registrations.len());

    for reg in registrations.iter_mut() {
        if reg.condition_gates.is_empty() {
            continue;
        }

        let gates = std::mem::take(&mut reg.condition_gates);

        let shared_flag = Arc::new(AtomicBool::new(false));
        let set_identity = reg.set.dyn_clone();
        let guard_node = reg.set.create_guard_node(gates, shared_flag.clone());

        guard_nodes.push((set_identity, shared_flag, guard_node));
    }
    for node in schedule_systems.iter_mut() {
        let sys = node.system_mut();
        let guard_ids: Vec<Box<dyn SystemIdentity>> = guard_nodes
            .iter()
            .map(|(_, _, g)| g.system().identity().dyn_clone())
            .collect();

        let orderings = sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);

        let mut matching_flags = Vec::new();
        for member_set in &orderings.member_of {
            for (idx, (guard_set, shared_flag, _)) in guard_nodes.iter().enumerate() {
                if member_set.dyn_eq(guard_set.as_ref()) {
                    matching_flags.push(shared_flag.clone());
                    let guard_id = guard_ids[idx].dyn_clone();
                    if !orderings
                        .run_after_systems
                        .iter()
                        .any(|id| id.dyn_system_eq(guard_id.as_ref()))
                    {
                        orderings.run_after_systems.push(guard_id);
                    }
                }
            }
        }

        if !matching_flags.is_empty() {
            let list = sys.get_or_init_mut::<RunConditionsList>(RunConditionsList::default);
            for flag in matching_flags.into_iter().rev() {
                let gate: ConditionFn = Box::new(move |_world| flag.load(Ordering::Acquire));
                list.insert(0, gate);
            }
        }
    }

    for reg in registrations.iter() {
        let mut target_before_systems: Vec<Box<dyn SystemIdentity>> = Vec::new();
        for sys_id in &reg.run_before_systems {
            target_before_systems.push(sys_id.dyn_clone());
        }
        for target_set in &reg.run_before_sets {
            for node in schedule_systems.iter_mut() {
                let sys = node.system_mut();
                let sys_id = sys.identity().dyn_clone();
                let orderings = sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);
                if orderings
                    .member_of
                    .iter()
                    .any(|s| s.dyn_eq(target_set.as_ref()))
                {
                    target_before_systems.push(sys_id);
                }
            }
            for (guard_set, _, guard_node) in &guard_nodes {
                if guard_set.dyn_eq(target_set.as_ref()) {
                    target_before_systems.push(guard_node.system().identity().dyn_clone());
                }
            }
        }

        let mut target_after_systems: Vec<Box<dyn SystemIdentity>> = Vec::new();
        for sys_id in &reg.run_after_systems {
            target_after_systems.push(sys_id.dyn_clone());
        }
        for target_set in &reg.run_after_sets {
            for node in schedule_systems.iter_mut() {
                let sys = node.system_mut();
                let sys_id = sys.identity().dyn_clone();
                let orderings = sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);
                if orderings
                    .member_of
                    .iter()
                    .any(|s| s.dyn_eq(target_set.as_ref()))
                {
                    target_after_systems.push(sys_id);
                }
            }
            for (guard_set, _, guard_node) in &guard_nodes {
                if guard_set.dyn_eq(target_set.as_ref()) {
                    target_after_systems.push(guard_node.system().identity().dyn_clone());
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
                for id in &target_before_systems {
                    if !orderings
                        .run_before_systems
                        .iter()
                        .any(|existing| existing.dyn_system_eq(id.as_ref()))
                    {
                        orderings.run_before_systems.push(id.dyn_clone());
                    }
                }
                for id in &target_after_systems {
                    if !orderings
                        .run_after_systems
                        .iter()
                        .any(|existing| existing.dyn_system_eq(id.as_ref()))
                    {
                        orderings.run_after_systems.push(id.dyn_clone());
                    }
                }
            }
        }
    }

    struct SystemSetMemberCache {
        identity: Box<dyn SystemIdentity>,
        member_of: Vec<Box<dyn SystemSet>>,
    }

    let mut member_cache: Vec<SystemSetMemberCache> = Vec::with_capacity(schedule_systems.len());
    for node in schedule_systems.iter_mut() {
        let sys = node.system_mut();
        let identity = sys.identity().dyn_clone();
        let orderings = sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);
        let member_of = orderings.member_of.iter().map(|s| s.dyn_clone()).collect();
        member_cache.push(SystemSetMemberCache {
            identity,
            member_of,
        });
    }

    for node in schedule_systems.iter_mut() {
        let sys = node.system_mut();
        let local_before_sets = {
            let orderings = sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);
            std::mem::take(&mut orderings.run_before_sets)
        };

        for target_set in local_before_sets {
            if !registrations
                .iter()
                .any(|reg| reg.set.dyn_eq(target_set.as_ref()))
            {
                panic!(
                    "❌ SystemSet Error: System '{}' is configured to run before a SystemSet which was never registered!",
                    sys.identity().display_name()
                );
            }

            let orderings = sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);
            for cache_item in &member_cache {
                if cache_item
                    .member_of
                    .iter()
                    .any(|s| s.dyn_eq(target_set.as_ref()))
                    && !orderings
                        .run_before_systems
                        .iter()
                        .any(|id| id.dyn_system_eq(cache_item.identity.as_ref()))
                {
                    orderings
                        .run_before_systems
                        .push(cache_item.identity.dyn_clone());
                }
            }
            for (guard_set, _, guard_node) in &guard_nodes {
                if guard_set.dyn_eq(target_set.as_ref()) {
                    let guard_id = guard_node.system().identity().dyn_clone();
                    if !orderings
                        .run_before_systems
                        .iter()
                        .any(|id| id.dyn_system_eq(guard_id.as_ref()))
                    {
                        orderings.run_before_systems.push(guard_id);
                    }
                }
            }
        }

        let local_after_sets = {
            let orderings = sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);
            std::mem::take(&mut orderings.run_after_sets)
        };

        for target_set in local_after_sets {
            if !registrations
                .iter()
                .any(|reg| reg.set.dyn_eq(target_set.as_ref()))
            {
                panic!(
                    "❌ SystemSet Error: System '{}' is configured to run after a SystemSet which was never registered!",
                    sys.identity().display_name()
                );
            }

            let orderings = sys.get_or_init_mut::<SystemOrderings>(SystemOrderings::default);
            for cache_item in &member_cache {
                if cache_item
                    .member_of
                    .iter()
                    .any(|s| s.dyn_eq(target_set.as_ref()))
                    && !orderings
                        .run_after_systems
                        .iter()
                        .any(|id| id.dyn_system_eq(cache_item.identity.as_ref()))
                {
                    orderings
                        .run_after_systems
                        .push(cache_item.identity.dyn_clone());
                }
            }
            for (guard_set, _, guard_node) in &guard_nodes {
                if guard_set.dyn_eq(target_set.as_ref()) {
                    let guard_id = guard_node.system().identity().dyn_clone();
                    if !orderings
                        .run_after_systems
                        .iter()
                        .any(|id| id.dyn_system_eq(guard_id.as_ref()))
                    {
                        orderings.run_after_systems.push(guard_id);
                    }
                }
            }
        }
    }

    for (_, _, guard_node) in guard_nodes {
        generated_guards.push(guard_node);
    }
}
