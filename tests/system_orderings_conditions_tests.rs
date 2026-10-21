use avenix::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SystemRunEvent {
    First,
    Second,
    Third,
    Fourth,
}

#[derive(Resource, Default)]
struct RunTracker {
    history: Vec<SystemRunEvent>,
}

#[derive(Resource, Default)]
struct Counter {
    value: u32,
}

fn first_system(mut tracker: ResMut<RunTracker>) {
    tracker.history.push(SystemRunEvent::First);
}

fn second_system(mut tracker: ResMut<RunTracker>) {
    tracker.history.push(SystemRunEvent::Second);
}

fn third_system(mut tracker: ResMut<RunTracker>) {
    tracker.history.push(SystemRunEvent::Third);
}

fn fourth_system(mut tracker: ResMut<RunTracker>) {
    tracker.history.push(SystemRunEvent::Fourth);
}

#[test_fork::test]
fn test_complex_scheduling_and_conditions() {
    let mut app = App::new();
    app.insert_resource(RunTracker::default())
        .insert_resource(Counter { value: 10 })
        .add_systems(
            Update,
            (
                first_system.after(second_system).before(third_system),
                second_system.run_if(|counter_access: &ParallelResourceAccessor<Counter>| {
                    counter_access.scope(|counter| counter.value == 10)
                }),
            )
                .run_if(|accessor: &ParallelMultiParam| {
                    accessor.counter_access.scope_mut(|mut counter| {
                        counter.value += 1;
                        counter.value -= 1;
                    });
                    accessor.run_tracker_accessor.is_present()
                }),
        )
        .add_systems(
            Update,
            (
                third_system.run_if(|counter_access: &ParallelResourceAccessor<Counter>| {
                    counter_access.scope(|counter| counter.value > 5)
                }),
                fourth_system.run_if(|counter_access: &ParallelResourceAccessor<Counter>| {
                    counter_access.scope(|counter| counter.value == 999)
                }),
            )
                .chain(),
        );

    app.build();
    app.run_startup();
    app.update();

    let tracker = app.world().resource::<RunTracker>();

    assert_eq!(
        tracker.history,
        vec![
            SystemRunEvent::Second,
            SystemRunEvent::First,
            SystemRunEvent::Third
        ]
    );
}

#[derive(ParallelSystemParam, SystemParam)]
struct ParallelMultiParam {
    counter_access: ParallelResourceAccessor<Counter>,
    run_tracker_accessor: ParallelResourceAccessor<RunTracker>,
}
