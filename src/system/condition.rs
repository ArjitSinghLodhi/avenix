use crate::{
    resources::{ParallelResourceAccessor, Resource},
    system::{ParallelSystemParam, SystemMeta, SystemParam},
    world::World,
};

pub type ConditionFn = Box<dyn Fn(&World) -> bool + Send + Sync>;

#[derive(Default)]
pub struct RunConditionsList {
    runtime_gates: Vec<ConditionFn>,
}

impl RunConditionsList {
    pub fn conditions(&self) -> &Vec<ConditionFn> {
        &self.runtime_gates
    }

    pub fn insert(&mut self, index: usize, condition: ConditionFn) {
        self.runtime_gates.insert(index, condition);
    }

    pub fn add_condition<C, T>(&mut self, cond: C)
    where
        C: Condition<T> + Send + Sync + 'static,
    {
        C::init_condition_access();
        let run_gate: ConditionFn = Box::new(move |world: &World| cond.run(world));
        self.runtime_gates.push(run_gate);
    }
}

/// A trait for runtime evaluation gates that filter system execution.
///
/// Unlike standard systems, conditions are completely decoupled from traditional scheduling
/// constraints. They are treated as pure, out-of-band evaluation flags that can be executed
/// concurrently across multiple threads at any arbitrary time. Because the scheduler provides
/// no order-of-execution or isolation guarantees for conditions, they should rely strictly
/// on thread-safe, non-blocking parallel parameter lookups.
pub trait Condition<T>: Send + Sync + 'static {
    fn init_condition_access();
    fn run(&self, world: &World) -> bool;
}

impl<F, P> Condition<P> for F
where
    F: Send + Sync + 'static,
    for<'world> P: SystemParam<Item<'world> = P> + ParallelSystemParam,
    for<'world> P::Item<'world>: ParallelSystemParam,
    for<'world> F: Fn(P) -> bool,
{
    fn init_condition_access() {
        let mut meta = SystemMeta::new("ConditionAccess".to_string());
        P::init_access(&mut meta);
    }

    fn run(&self, world: &World) -> bool {
        if let Some(param_data) = P::get_param(world) {
            (self)(param_data)
        } else {
            false
        }
    }
}

pub fn not<P, C>(condition: C) -> impl Fn(&World) -> bool + Clone + Send + Sync
where
    P: 'static,
    C: Condition<P> + 'static + Clone,
{
    move |world: &World| !condition.run(world)
}

pub fn resource_exists<R>() -> impl Fn(&World) -> bool + Clone + Send + Sync
where
    R: Resource + Send + Sync,
{
    move |world: &World| {
        if let Some(resource_accessor) =
            <ParallelResourceAccessor<R> as SystemParam>::get_param(world)
        {
            resource_accessor.is_present()
        } else {
            false
        }
    }
}
