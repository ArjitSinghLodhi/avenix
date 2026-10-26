use avenix::prelude::*;

#[allow(dead_code)]
#[derive(Component)]
struct Position {
    x: f32,
    y: f32,
}
#[allow(dead_code)]
#[derive(Component)]
struct Velocity {
    x: f32,
    y: f32,
}

#[derive(Resource)]
struct FrameCounter {
    current_frame: u32,
}

fn simulate_frame_2_mutation(mut query: Query<&mut Position>) {
    for mut pos in query.iter_mut() {
        pos.x = 999.0;
    }
}

fn verify_frame_1_baseline(changed_query: Query<&Position, Changed<Position>>) {
    let changed_count = changed_query.filtered_entities();
    assert_eq!(changed_count, 0);
}

fn verify_frame_2_immediate_change(changed_query: Query<&Position, Changed<Position>>) {
    let changed_count = changed_query.filtered_entities();
    assert_eq!(changed_count, 1);
    for pos in changed_query.iter() {
        assert_eq!(pos.x, 999.0);
    }
}

fn verify_frame_3_persistent_state(
    changed_query: Query<&Position, Changed<Position>>,
    tracker_query: Query<(&Position, ChangedTracker<Velocity>)>,
) {
    let changed_count = changed_query.filtered_entities();
    assert_eq!(changed_count, 1);
    for (pos, tracker) in tracker_query.iter() {
        assert_eq!(pos.x, 999.0);
        assert!(!tracker.is_changed());
    }
}

fn verify_frame_4_decayed_state(changed_query: Query<&Position, Changed<Position>>) {
    let changed_count = changed_query.filtered_entities();
    assert_eq!(changed_count, 0);
}

fn multi_frame_test_runner(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
    app.update();
    app.update();
    app.update();
}

#[test_fork::test]
fn test_multi_frame_change_propagation() {
    let mut app = App::new();
    app.insert_resource(FrameCounter { current_frame: 0 })
        .add_systems(Startup, spawn_tracking_entity)
        .add_systems(
            Update,
            (
                increment_frame_system,
                simulate_frame_2_mutation.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 2)
                    },
                ),
                verify_frame_1_baseline.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 1)
                    },
                ),
                verify_frame_2_immediate_change.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 2)
                    },
                ),
                verify_frame_3_persistent_state.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 3)
                    },
                ),
                verify_frame_4_decayed_state.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 4)
                    },
                ),
            )
                .chain(),
        );

    app.set_runner(multi_frame_test_runner);
    app.run();
}

fn increment_frame_system(mut counter: ResMut<FrameCounter>) {
    counter.current_frame += 1;
}

fn spawn_tracking_entity(commands: Commands) {
    commands.spawn((Position { x: 5.0, y: 5.0 }, Velocity { x: 1.0, y: 1.0 }));
}

fn modify_component_system(mut query: Query<&mut Position>) {
    for mut pos in query.iter_mut() {
        pos.x = 50.0;
    }
}

fn verify_change_tracking(frame: Res<FrameCounter>, query: Query<&Position, Changed<Position>>) {
    if frame.current_frame == 2 {
        let mut change_detected = false;
        for pos in query.iter() {
            if pos.x == 50.0 {
                change_detected = true;
            }
        }
        assert!(change_detected);
    }
}

fn verify_change_tracking_data(
    frame: Res<FrameCounter>,
    query: Query<(&Position, ChangedTracker<Position>)>,
) {
    if frame.current_frame == 2 {
        let mut change_detected_track = false;
        for (pos, tracker) in query.iter() {
            if pos.x == 50.0 && tracker.is_changed() {
                change_detected_track = true;
            }
        }
        assert!(change_detected_track);
    }
}

fn test_runner_once(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
    app.update();
    app.update();
}

#[test_fork::test]
fn test_changed_generational_tracking() {
    let mut app = App::new();
    app.insert_resource(FrameCounter { current_frame: 0 })
        .add_systems(Startup, spawn_tracking_entity)
        .add_systems(
            Update,
            (
                increment_frame_system,
                modify_component_system.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 1)
                    },
                ),
                verify_change_tracking,
                verify_change_tracking_data,
            )
                .chain(),
        );

    app.set_runner(test_runner_once);
    app.run();
}

fn setup_multi_frame_entities(commands: Commands) {
    commands.spawn((Position { x: 5.0, y: 5.0 }, Velocity { x: 1.0, y: 1.0 }));
}

fn midstream_mutator_system(mut query: Query<&mut Position>) {
    for mut pos in query.iter_mut() {
        pos.x = 50.0;
    }
}

fn test_runner_four_frames(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
    app.update();
    app.update();
    app.update();
}

fn complex_filter_verify_frame_1(
    not_changed_q: Query<&Position, Not<Changed<Position>>>,
    not_or_q: Query<&Position, Not<Or<(Changed<Position>, Changed<Velocity>)>>>,
    or_mixed_q: Query<&Position, Or<(Changed<Position>, With<Velocity>)>>,
) {
    assert_eq!(
        not_changed_q.filtered_entities(),
        1,
        "❌ Not<Changed> failed on frame 1 baseline!"
    );
    assert_eq!(
        not_or_q.filtered_entities(),
        1,
        "❌ Not<Or<...>> failed on frame 1 baseline!"
    );
    assert_eq!(
        or_mixed_q.filtered_entities(),
        1,
        "❌ Or<Changed, With> structural arm failed on frame 1 baseline!"
    );
}

fn complex_filter_verify_frame_3(
    not_changed_q: Query<&Position, Not<Changed<Position>>>,
    not_or_q: Query<&Position, Not<Or<(Changed<Position>, Changed<Velocity>)>>>,
    or_mixed_q: Query<&Position, Or<(Changed<Position>, With<Velocity>)>>,
) {
    assert_eq!(
        not_changed_q.filtered_entities(),
        0,
        "❌ Not<Changed> failed to filter out mutated entity on frame 3!"
    );
    assert_eq!(
        not_or_q.filtered_entities(),
        0,
        "❌ Not<Or<...>> failed to filter out mutated entity on frame 3!"
    );
    assert_eq!(
        or_mixed_q.filtered_entities(),
        1,
        "❌ Or<Changed, With> dynamic arm failed to catch matching entity on frame 3!"
    );
}

fn complex_filter_verify_frame_4(
    not_changed_q: Query<&Position, Not<Changed<Position>>>,
    not_or_q: Query<&Position, Not<Or<(Changed<Position>, Changed<Velocity>)>>>,
    or_mixed_q: Query<&Position, Or<(Changed<Position>, With<Velocity>)>>,
) {
    assert_eq!(
        not_changed_q.filtered_entities(),
        1,
        "❌ Not<Changed> failed to reset after change stagnation on frame 4!"
    );
    assert_eq!(
        not_or_q.filtered_entities(),
        1,
        "❌ Not<Or<...>> failed to reset after change stagnation on frame 4!"
    );
    assert_eq!(
        or_mixed_q.filtered_entities(),
        1,
        "❌ Or<Changed, With> dropped evaluation via structural arm on frame 4!"
    );
}

#[test_fork::test]
fn test_complex_changed_combinator_filters() {
    let mut app = App::new();
    app.insert_resource(FrameCounter { current_frame: 0 })
        .add_systems(Startup, setup_multi_frame_entities)
        .add_systems(
            Update,
            (
                increment_frame_system,
                midstream_mutator_system.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 2)
                    },
                ),
                complex_filter_verify_frame_1.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 1)
                    },
                ),
                complex_filter_verify_frame_3.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 3)
                    },
                ),
                complex_filter_verify_frame_4.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 4)
                    },
                ),
            )
                .chain(),
        )
        .set_runner(test_runner_four_frames);
    app.run();
}
