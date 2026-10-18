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

fn increment_frame_system(mut counter: ResMut<FrameCounter>) {
    counter.current_frame += 1;
}

fn setup_multi_frame_entities(commands: Commands) {
    commands.spawn((Position { x: 10.0, y: 10.0 }, Velocity { x: 1.0, y: 1.0 }));
}

fn pre_mutation_verify_system(
    counter: Res<FrameCounter>,
    pos_changed_query: Query<&Position, Changed<Position>>,
    vel_changed_query: Query<&Velocity, Changed<Velocity>>,
    pos_tracker_query: Query<ChangedTracker<Position>>,
    vel_tracker_query: Query<ChangedTracker<Velocity>>,
) {
    let pos_changes = pos_changed_query.total_entities();
    let vel_changes = vel_changed_query.total_entities();

    let mut pos_tracker_changes = 0;
    for tracker in pos_tracker_query.iter() {
        if tracker.is_changed() {
            pos_tracker_changes += 1;
        }
    }

    let mut vel_tracker_changes = 0;
    for tracker in vel_tracker_query.iter() {
        if tracker.is_changed() {
            vel_tracker_changes += 1;
        }
    }

    if counter.current_frame == 1 {
        assert_eq!(
            pos_changes, 0,
            "❌ Soundness Failure: Spawned Position registered as changed on Changed Filter Frame 1 baseline!"
        );
        assert_eq!(
            vel_changes, 0,
            "❌ Soundness Failure: Spawned Velocity registered as changed on Changed Filter Frame 1 baseline!"
        );
        assert_eq!(
            pos_tracker_changes, 0,
            "❌ Soundness Failure: Spawned Position registered as changed on ChangedTracker Frame 1 baseline!"
        );
        assert_eq!(
            vel_tracker_changes, 0,
            "❌ Soundness Failure: Spawned Velocity registered as changed on ChangedTracker Frame 1 baseline!"
        );
    }

    if counter.current_frame == 2 {
        assert_eq!(
            pos_changes, 0,
            "❌ Pipeline Leak: Pre-mutator system saw a Changed Filter Position change prematurely on Frame 2!"
        );
        assert_eq!(
            vel_changes, 0,
            "❌ Pipeline Leak: Pre-mutator system saw a Changed Filter Velocity change prematurely on Frame 2!"
        );
        assert_eq!(
            pos_tracker_changes, 0,
            "❌ Pipeline Leak: Pre-mutator system saw a ChangedTracker Position change prematurely on Frame 2!"
        );
        assert_eq!(
            vel_tracker_changes, 0,
            "❌ Pipeline Leak: Pre-mutator system saw a ChangedTracker Velocity change prematurely on Frame 2!"
        );
    }

    if counter.current_frame == 3 {
        assert_eq!(
            pos_changes, 1,
            "❌ Core Failure: Pre-mutator failed to detect independent Position historical mutation on Changed Filter Frame 3!"
        );
        assert_eq!(
            vel_changes, 1,
            "❌ Core Failure: Pre-mutator failed to detect independent Velocity historical mutation on Changed Filter Frame 3!"
        );
        assert_eq!(
            pos_tracker_changes, 1,
            "❌ Core Failure: Pre-mutator failed to detect independent Position historical mutation on ChangedTracker Frame 3!"
        );
        assert_eq!(
            vel_tracker_changes, 1,
            "❌ Core Failure: Pre-mutator failed to detect independent Velocity historical mutation on ChangedTracker Frame 3!"
        );
    }

    if counter.current_frame == 4 {
        assert_eq!(
            pos_changes, 0,
            "❌ Stagnation Failure: Stale Position change marker leaked into Changed Filter Frame 4 for Pre-system!"
        );
        assert_eq!(
            vel_changes, 0,
            "❌ Stagnation Failure: Stale Velocity change marker leaked into Changed Filter Frame 4 for Pre-system!"
        );
        assert_eq!(
            pos_tracker_changes, 0,
            "❌ Stagnation Failure: Stale Position change marker leaked into ChangedTracker Frame 4 for Pre-system!"
        );
        assert_eq!(
            vel_tracker_changes, 0,
            "❌ Stagnation Failure: Stale Velocity change marker leaked into ChangedTracker Frame 4 for Pre-system!"
        );
    }
}

fn midstream_mutator_system(
    mut pos_query: Query<&mut Position>,
    mut vel_query: Query<&mut Velocity>,
) {
    for mut pos in pos_query.iter_mut() {
        *pos = Position { x: 20.0, y: 10.0 };
    }

    for mut vel in vel_query.iter_mut() {
        *vel = Velocity { x: 5.0, y: 1.0 };
    }
}

fn post_mutation_verify_system(
    counter: Res<FrameCounter>,
    pos_changed_query: Query<&Position, Changed<Position>>,
    vel_changed_query: Query<&Velocity, Changed<Velocity>>,
    pos_tracker_query: Query<ChangedTracker<Position>>,
    vel_tracker_query: Query<ChangedTracker<Velocity>>,
) {
    let pos_changes = pos_changed_query.total_entities();
    let vel_changes = vel_changed_query.total_entities();

    let mut pos_tracker_changes = 0;
    for tracker in pos_tracker_query.iter() {
        if tracker.is_changed() {
            pos_tracker_changes += 1;
        }
    }

    let mut vel_tracker_changes = 0;
    for tracker in vel_tracker_query.iter() {
        if tracker.is_changed() {
            vel_tracker_changes += 1;
        }
    }

    if counter.current_frame == 1 {
        assert_eq!(
            pos_changes, 0,
            "❌ Soundness Failure: Post-system flagged Changed Filter Position change on Frame 1 baseline."
        );
        assert_eq!(
            vel_changes, 0,
            "❌ Soundness Failure: Post-system flagged Changed Filter Velocity change on Frame 1 baseline."
        );
        assert_eq!(
            pos_tracker_changes, 0,
            "❌ Soundness Failure: Post-system flagged ChangedTracker Position change on Frame 1 baseline."
        );
        assert_eq!(
            vel_tracker_changes, 0,
            "❌ Soundness Failure: Post-system flagged ChangedTracker Velocity change on Frame 1 baseline."
        );
    }

    if counter.current_frame == 2 {
        assert_eq!(
            pos_changes, 0,
            "❌ Buffer Break: Post-mutator caught an immediate inline Changed Filter Position alteration on Frame 2!"
        );
        assert_eq!(
            vel_changes, 0,
            "❌ Buffer Break: Post-mutator caught an immediate inline Changed Filter Velocity alteration on Frame 2!"
        );
        assert_eq!(
            pos_tracker_changes, 0,
            "❌ Buffer Break: Post-mutator caught an immediate inline ChangedTracker Position alteration on Frame 2!"
        );
        assert_eq!(
            vel_tracker_changes, 0,
            "❌ Buffer Break: Post-mutator caught an immediate inline ChangedTracker Velocity alteration on Frame 2!"
        );
    }

    if counter.current_frame == 3 {
        assert_eq!(
            pos_changes, 1,
            "❌ Pipeline Leak: Post-mutator failed to detect historical Changed Filter Position change on Frame 3!"
        );
        assert_eq!(
            vel_changes, 1,
            "❌ Pipeline Leak: Post-mutator failed to detect historical Changed Filter Velocity change on Frame 3!"
        );
        assert_eq!(
            pos_tracker_changes, 1,
            "❌ Pipeline Leak: Post-mutator failed to detect historical ChangedTracker Position change on Frame 3!"
        );
        assert_eq!(
            vel_tracker_changes, 1,
            "❌ Pipeline Leak: Post-mutator failed to detect historical ChangedTracker Velocity change on Frame 3!"
        );
    }

    if counter.current_frame == 4 {
        assert_eq!(
            pos_changes, 0,
            "❌ Tracking Error: Post-system leaked Changed Filter Position change into Frame 4 (should stagnate)."
        );
        assert_eq!(
            vel_changes, 0,
            "❌ Tracking Error: Post-system leaked Changed Filter Velocity change into Frame 4 (should stagnate)."
        );
        assert_eq!(
            pos_tracker_changes, 0,
            "❌ Tracking Error: Post-system leaked ChangedTracker Position change into Frame 4 (should stagnate)."
        );
        assert_eq!(
            vel_tracker_changes, 0,
            "❌ Tracking Error: Post-system leaked ChangedTracker Velocity change into Frame 4 (should stagnate)."
        );
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

#[test_fork::test]
fn test_multiple_changed_filters_lifecycle() {
    let mut app = App::new();

    app.insert_resource(FrameCounter { current_frame: 0 })
        .add_systems(Startup, setup_multi_frame_entities);

    app.add_systems(
        Update,
        (
            increment_frame_system,
            pre_mutation_verify_system,
            midstream_mutator_system.run_if(|counter: &ParallelResourceAccessor<FrameCounter>| {
                counter.scope(|c| c.current_frame == 2)
            }),
            post_mutation_verify_system,
        )
            .chain(),
    );

    app.set_runner(test_runner_four_frames);
    app.run();
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
    query: Query<(&Position, ChangedTracker<Velocity>)>,
) {
    if frame.current_frame == 2 {
        let mut change_detected_track = false;
        for (pos, mark) in query.iter() {
            if pos.x == 50.0 && !mark.is_changed() {
                change_detected_track = true;
            }
        }
        assert!(change_detected_track);
    }
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
                modify_component_system,
                verify_change_tracking,
                verify_change_tracking_data,
            )
                .chain(),
        );

    app.set_runner(test_runner_once);
    app.run();
}

fn test_runner_once(app: &mut App) {
    app.build();
    app.run_startup();
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
        not_changed_q.total_entities(),
        1,
        "❌ Not<Changed> failed on frame 1 baseline!"
    );
    assert_eq!(
        not_or_q.total_entities(),
        1,
        "❌ Not<Or<...>> failed on frame 1 baseline!"
    );
    assert_eq!(
        or_mixed_q.total_entities(),
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
        not_changed_q.total_entities(),
        0,
        "❌ Not<Changed> failed to filter out mutated entity on frame 3!"
    );
    assert_eq!(
        not_or_q.total_entities(),
        0,
        "❌ Not<Or<...>> failed to filter out mutated entity on frame 3!"
    );
    assert_eq!(
        or_mixed_q.total_entities(),
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
        not_changed_q.total_entities(),
        1,
        "❌ Not<Changed> failed to reset after change stagnation on frame 4!"
    );
    assert_eq!(
        not_or_q.total_entities(),
        1,
        "❌ Not<Or<...>> failed to reset after change stagnation on frame 4!"
    );
    assert_eq!(
        or_mixed_q.total_entities(),
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
                    |counter: &ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 2)
                    },
                ),
                complex_filter_verify_frame_1.run_if(
                    |counter: &ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 1)
                    },
                ),
                complex_filter_verify_frame_3.run_if(
                    |counter: &ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 3)
                    },
                ),
                complex_filter_verify_frame_4.run_if(
                    |counter: &ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 4)
                    },
                ),
            )
                .chain(),
        )
        .set_runner(test_runner_four_frames);
    app.run();
}
