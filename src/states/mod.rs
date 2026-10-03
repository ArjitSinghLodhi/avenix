use crate::resources::ParallelResourceAccessor;
use crate::system::condition::Condition;
use crate::{
    app::App,
    resources::{ResMut, Resource},
    schedule::PreUpdate,
};
use std::fmt::Debug;

pub trait States: PartialEq + Eq + Send + Sync + Clone + Debug + 'static {}

pub struct State<T: States> {
    current: T,
    last: T,
    just_changed: bool,
}

impl<T: States> Resource for State<T> {}

impl<T: States> State<T> {
    pub fn current(&self) -> &T {
        &self.current
    }

    pub fn last(&self) -> &T {
        &self.last
    }

    pub fn just_changed(&self) -> bool {
        self.just_changed
    }
}

pub struct NextState<T: States> {
    state: Option<T>,
}

impl<T: States> Resource for NextState<T> {}

impl<T: States> NextState<T> {
    pub fn set(&mut self, next_state: T) {
        self.state = Some(next_state);
    }

    pub fn clear(&mut self) {
        self.state = None;
    }

    pub fn current_next_state(&self) -> Option<&T> {
        self.state.as_ref()
    }
}

fn state_transition_system<T: States>(
    mut state: ResMut<State<T>>,
    mut next_state: ResMut<NextState<T>>,
) {
    state.just_changed = false;
    if let Some(next) = next_state.state.take()
        && state.current != next
    {
        state.last = state.current.clone();
        state.current = next;
        state.just_changed = true;
    }
}

pub(crate) fn setup_states_schedules_and_systems<T: States>(app: &mut App, state: T) {
    app.add_systems(PreUpdate, state_transition_system::<T>)
        .insert_resource(State {
            current: state.clone(),
            last: state.clone(),
            just_changed: false,
        })
        .insert_resource(NextState::<T> { state: None });
}

pub fn in_state<T: States>(
    expected: T,
) -> impl Condition<ParallelResourceAccessor<State<T>>, ConditionData = ParallelResourceAccessor<State<T>>>
{
    move |state_accessor: &ParallelResourceAccessor<State<T>>| {
        state_accessor.scope(|state| *state.current() == expected)
    }
}

pub fn entered_state<T: States>(
    state: T,
) -> impl Condition<ParallelResourceAccessor<State<T>>, ConditionData = ParallelResourceAccessor<State<T>>>
{
    move |state_accessor: &ParallelResourceAccessor<State<T>>| {
        state_accessor.scope(|res_state| *res_state.current() == state && res_state.just_changed())
    }
}

pub fn exited_state<T: States>(
    state: T,
) -> impl Condition<ParallelResourceAccessor<State<T>>, ConditionData = ParallelResourceAccessor<State<T>>>
{
    move |state_accessor: &ParallelResourceAccessor<State<T>>| {
        state_accessor.scope(|res_state| *res_state.last() == state && res_state.just_changed())
    }
}
