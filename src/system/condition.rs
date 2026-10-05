use crate::{extensions::World, system::ParallelSystemParam};

pub type ConditionBuilder = Box<dyn FnOnce(&mut World) -> ConditionFn + Send + Sync>;
pub type ConditionFn = Box<dyn Fn() -> bool + Send + Sync>;

#[derive(Default)]
pub struct RunConditionsList {
    builders: Vec<ConditionBuilder>,
    runtime_gates: Vec<ConditionFn>,
}

impl RunConditionsList {
    pub fn conditions(&self) -> &Vec<ConditionFn> {
        &self.runtime_gates
    }

    pub fn conditions_mut(&mut self) -> &mut Vec<ConditionFn> {
        &mut self.runtime_gates
    }

    pub fn add_condition<C, T>(&mut self, cond: C)
    where
        C: Condition<T> + Send + Sync + 'static,
    {
        let builder = Box::new(move |world: &mut World| {
            let typed_data = cond.init_condition_data(world);
            let run_gate: ConditionFn = Box::new(move || cond.run(&typed_data));
            run_gate
        });

        self.builders.push(builder);
    }

    pub fn build_conditions(&mut self, world: &mut World) {
        for builder in self.builders.drain(..) {
            let compiled_gate = (builder)(world);
            self.runtime_gates.push(compiled_gate);
        }
    }
}

pub trait Condition<T>: Send + Sync + 'static {
    type ConditionData: Send + Sync + 'static;
    fn init_condition_data(&self, world: &World) -> Self::ConditionData;
    fn run(&self, data: &Self::ConditionData) -> bool;
}

impl<F, P> Condition<P> for F
where
    F: Fn(&P) -> bool + Send + Sync + 'static,
    P: ParallelSystemParam,
{
    type ConditionData = P;
    fn init_condition_data(&self, world: &World) -> Self::ConditionData {
        P::get_param(world)
    }

    fn run(&self, data: &Self::ConditionData) -> bool {
        (self)(data)
    }
}

pub fn not<P, C>(condition: C) -> impl Condition<P, ConditionData = P>
where
    P: 'static + ParallelSystemParam,
    C: Condition<P, ConditionData = P> + 'static,
{
    let condition = condition;
    move |param: &P| !condition.run(param)
}
