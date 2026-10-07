use avenix::prelude::*;

#[derive(Resource, Default)]
struct RunTracker {
    history: Vec<&'static str>,
}

#[derive(Resource, Default)]
struct Counter {
    value: u32,
}

fn first_system(mut tracker: ResMut<RunTracker>) {
    tracker.history.push("first");
}

fn second_system(mut tracker: ResMut<RunTracker>) {
    tracker.history.push("second");
}

fn third_system(mut tracker: ResMut<RunTracker>) {
    tracker.history.push("third");
}

fn fourth_system(mut tracker: ResMut<RunTracker>) {
    tracker.history.push("fourth");
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

    let tracker = app.world().get_resource::<RunTracker>();

    assert_eq!(tracker.history, vec!["second", "first", "third"]);
}

#[derive(ParallelSystemParam, SystemParam)]
struct ParallelMultiParam {
    counter_access: ParallelResourceAccessor<Counter>,
    run_tracker_accessor: ParallelResourceAccessor<RunTracker>,
}
