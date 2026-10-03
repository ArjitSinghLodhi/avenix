use avenix::prelude::*;

#[derive(Resource, Default)]
struct RunTracker {
    history: Vec<&'static str>,
}

#[derive(Resource, Default)]
struct Counter {
    value: u32,
}

#[derive(Resource, Default)]
struct Diagnostics {
    frame_count: u64,
}

fn first_system(mut tracker: ResMut<RunTracker>) {
    tracker.history.push("first");
    println!("Executed: first_system");
}

fn second_system(mut tracker: ResMut<RunTracker>) {
    tracker.history.push("second");
    println!("Executed: second_system");
}

fn third_system(mut tracker: ResMut<RunTracker>) {
    tracker.history.push("third");
    println!("Executed: third_system");
}

fn fourth_system(mut tracker: ResMut<RunTracker>) {
    tracker.history.push("fourth");
    println!("Executed: fourth_system");
}

#[derive(ParallelSystemParam, SystemParam)]
struct DebugMetricsParam {
    diagnostics_access: ParallelResourceAccessor<Diagnostics>,
}

#[derive(ParallelSystemParam, SystemParam)]
struct ParallelMultiParam {
    counter_access: ParallelResourceAccessor<Counter>,
    run_tracker_accessor: ParallelResourceAccessor<RunTracker>,
    metrics: DebugMetricsParam,
}

fn main() {
    println!("--- Setting up Avenix ECS Parallel Validation Pipeline ---");

    let mut app = App::new();
    app.insert_resource(RunTracker::default())
        .insert_resource(Counter { value: 10 })
        .insert_resource(Diagnostics { frame_count: 1 })
        .add_systems(
            Update,
            (
                first_system.after(second_system).before(third_system),
                second_system.run_if(|counter_access: &ParallelResourceAccessor<Counter>| {
                    counter_access.scope(|counter| counter.value == 10)
                }),
            )
                .run_if(|accessor: &ParallelMultiParam| {
                    accessor.metrics.diagnostics_access.scope(|diag| {
                        println!("Evaluating conditions at frame: {}", diag.frame_count);
                    });

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

    println!("Building and initializing application schedules...");
    app.build();
    app.run_startup();

    println!("Running frame update step...");
    app.update();

    let tracker = app.world().get_resource::<RunTracker>();
    println!("\nFinal System Execution History: {:?}", tracker.history);

    assert_eq!(tracker.history, vec!["second", "first", "third"]);
    println!("Execution pipeline successfully validated with nested parallel system parameters!");
}
