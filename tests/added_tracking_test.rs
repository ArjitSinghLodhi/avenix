use avenix::prelude::*;
use rusty_fork::rusty_fork_test;

#[derive(PartialEq, Component)]
enum EntityTag {
    AdderTarget,
    InserterTarget,
    DenseNeighbor,
}

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
#[allow(dead_code)]
#[derive(Component)]
struct Acceleration {
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

fn setup_double_buffer_entities(mut commands: Commands) {
    commands.spawn((Position { x: 1.0, y: 1.0 }, EntityTag::AdderTarget));
    commands.spawn((Position { x: 2.0, y: 2.0 }, EntityTag::InserterTarget));
    commands.spawn((Position { x: 3.0, y: 3.0 }, EntityTag::DenseNeighbor));
}

fn apply_frame_1_mutations(
    counter: Res<FrameCounter>,
    query: Query<(Entity, &EntityTag)>,
    mut commands: Commands,
) {
    if counter.current_frame != 1 {
        return;
    }

    for view in query.iter() {
        for (entity, tag) in view.iter() {
            match tag {
                EntityTag::AdderTarget => {
                    commands.add_components(entity.clone(), Velocity { x: 10.0, y: 10.0 });
                }
                EntityTag::InserterTarget => {
                    commands.insert_components(entity.clone(), Acceleration { x: 5.0, y: 5.0 });
                }
                EntityTag::DenseNeighbor => {}
            }
        }
    }
}

fn verify_frame_1_buffering_isolation(
    counter: Res<FrameCounter>,
    query_added_vel: Query<&EntityTag, Added<Velocity>>,
    query_added_accel: Query<&EntityTag, Added<Acceleration>>,
) {
    if counter.current_frame != 1 {
        return;
    }

    let mut vel_visible_early = false;
    for view in query_added_vel.iter() {
        for _ in view.iter() {
            vel_visible_early = true;
        }
    }
    assert!(
        !vel_visible_early,
        "Double buffer leak! Added<Velocity> visible on write frame."
    );

    let mut accel_visible_early = false;
    for view in query_added_accel.iter() {
        for _ in view.iter() {
            accel_visible_early = true;
        }
    }
    assert!(
        !accel_visible_early,
        "Double buffer leak! Added<Acceleration> visible on write frame."
    );
}

fn verify_frame_2_buffered_reads(
    counter: Res<FrameCounter>,
    query_vel: Query<&EntityTag, Added<Velocity>>,
    query_accel: Query<&EntityTag, Added<Acceleration>>,
    query_neighbor: Query<&EntityTag, (With<Position>, Without<Velocity>, Without<Acceleration>)>,
) {
    if counter.current_frame != 2 {
        return;
    }

    let mut vel_detected = false;
    for view in query_vel.iter() {
        for tag in view.iter() {
            if *tag == EntityTag::AdderTarget {
                vel_detected = true;
            }
        }
    }
    assert!(
        vel_detected,
        "Buffered .add_components failed to surface on Frame 2."
    );

    let mut accel_detected = false;
    for view in query_accel.iter() {
        for tag in view.iter() {
            if *tag == EntityTag::InserterTarget {
                accel_detected = true;
            }
        }
    }
    assert!(
        accel_detected,
        "Buffered .insert_components failed to surface on Frame 2."
    );

    let mut neighbor_intact = false;
    for view in query_neighbor.iter() {
        for tag in view.iter() {
            if *tag == EntityTag::DenseNeighbor {
                neighbor_intact = true;
            }
        }
    }
    assert!(
        neighbor_intact,
        "Archetype neighbor layout was corrupted during buffered swaps."
    );
}

fn apply_frame_2_redundant_insert(
    counter: Res<FrameCounter>,
    query: Query<(Entity, &EntityTag)>,
    mut commands: Commands,
) {
    if counter.current_frame != 2 {
        return;
    }

    for view in query.iter() {
        for (entity, tag) in view.iter() {
            if *tag == EntityTag::InserterTarget {
                commands.insert_components(entity.clone(), Acceleration { x: 99.0, y: 99.0 });
            }
        }
    }
}

fn verify_frame_3_decay_and_overwrites(
    counter: Res<FrameCounter>,
    query_vel: Query<&EntityTag, Added<Velocity>>,
    query_accel: Query<&EntityTag, Added<Acceleration>>,
) {
    if counter.current_frame != 3 {
        return;
    }

    let mut vel_active = false;
    for view in query_vel.iter() {
        for _ in view.iter() {
            vel_active = true;
        }
    }
    assert!(
        !vel_active,
        "Added<Velocity> failed to naturally decay after its visibility frame."
    );

    let mut accel_retriggered = false;
    for view in query_accel.iter() {
        for _ in view.iter() {
            accel_retriggered = true;
        }
    }
    assert!(
        !accel_retriggered,
        "Redundant .insert_components mistakenly re-flipped the Added tracking state!"
    );
}

rusty_fork_test! {
    #[test]
    fn test_double_buffered_add_and_insert() {
        let mut app = App::new();
        app.add_plugins(DefaultSchedulesPlugin)
            .insert_resource(FrameCounter { current_frame: 0 })
            .add_systems(Startup, setup_double_buffer_entities)
            .add_systems(
                Update,
                (
                    increment_frame_system,
                    apply_frame_1_mutations,
                    verify_frame_1_buffering_isolation,
                    verify_frame_2_buffered_reads,
                    apply_frame_2_redundant_insert,
                    verify_frame_3_decay_and_overwrites,
                ),
            );

        app.set_runner(test_runner_double_buffered);
        app.run();
    }
}

fn test_runner_double_buffered(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
    app.update();
    app.update();
}
