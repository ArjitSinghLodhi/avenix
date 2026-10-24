use avenix::prelude::*;

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

fn setup_double_buffer_entities(commands: Commands) {
    commands.spawn((Position { x: 1.0, y: 1.0 }, EntityTag::AdderTarget));
    commands.spawn((Position { x: 2.0, y: 2.0 }, EntityTag::InserterTarget));
    commands.spawn((Position { x: 3.0, y: 3.0 }, EntityTag::DenseNeighbor));
}

fn apply_frame_1_mutations(query: Query<(Entity, &EntityTag)>, commands: Commands) {
    for (entity, tag) in query.iter() {
        let entity_cmd = commands.entity(entity);
        match tag {
            EntityTag::AdderTarget => {
                entity_cmd.add(Velocity { x: 10.0, y: 10.0 });
            }
            EntityTag::InserterTarget => {
                entity_cmd.insert(Acceleration { x: 5.0, y: 5.0 });
            }
            EntityTag::DenseNeighbor => {}
        }
    }
}

fn verify_frame_1_buffering_isolation(
    query_added_vel: Query<&EntityTag, Added<Velocity>>,
    query_added_accel: Query<&EntityTag, Added<Acceleration>>,
) {
    let mut vel_visible_early = false;
    for _ in query_added_vel.iter() {
        vel_visible_early = true;
    }

    assert!(
        !vel_visible_early,
        "Double buffer leak! Added<Velocity> visible on write frame."
    );

    let mut accel_visible_early = false;
    for _ in query_added_accel.iter() {
        accel_visible_early = true;
    }

    assert!(
        !accel_visible_early,
        "Double buffer leak! Added<Acceleration> visible on write frame."
    );
}

fn verify_frame_2_buffered_reads(
    query_vel: Query<&EntityTag, Added<Velocity>>,
    query_accel: Query<&EntityTag, Added<Acceleration>>,
    query_neighbor: Query<&EntityTag, (With<Position>, Without<Velocity>, Without<Acceleration>)>,
) {
    let mut vel_detected = false;
    for tag in query_vel.iter() {
        if *tag == EntityTag::AdderTarget {
            vel_detected = true;
        }
    }

    assert!(
        vel_detected,
        "Buffered .add_components failed to surface on Frame 2."
    );

    let mut accel_detected = false;
    for tag in query_accel.iter() {
        if *tag == EntityTag::InserterTarget {
            accel_detected = true;
        }
    }

    assert!(
        accel_detected,
        "Buffered .insert_components failed to surface on Frame 2."
    );

    let mut neighbor_intact = false;
    for tag in query_neighbor.iter() {
        if *tag == EntityTag::DenseNeighbor {
            neighbor_intact = true;
        }
    }

    assert!(
        neighbor_intact,
        "Archetype neighbor layout was corrupted during buffered swaps."
    );
}

fn apply_frame_2_redundant_insert(query: Query<(Entity, &EntityTag)>, commands: Commands) {
    for (entity, tag) in query.iter() {
        if *tag == EntityTag::InserterTarget {
            commands
                .entity(entity)
                .insert(Acceleration { x: 99.0, y: 99.0 });
        }
    }
}

fn verify_frame_3_decay_and_overwrites(
    query_vel: Query<&EntityTag, Added<Velocity>>,
    query_accel: Query<&EntityTag, Added<Acceleration>>,
) {
    let mut vel_active = false;
    for _ in query_vel.iter() {
        vel_active = true;
    }

    assert!(
        !vel_active,
        "Added<Velocity> failed to naturally decay after its visibility frame."
    );

    let mut accel_retriggered = false;
    for _ in query_accel.iter() {
        accel_retriggered = true;
    }
    assert!(
        !accel_retriggered,
        "Redundant .insert_components mistakenly re-flipped the Added tracking state!"
    );
}

#[test_fork::test]
fn test_double_buffered_add_and_insert() {
    let mut app = App::new();
    app.insert_resource(FrameCounter { current_frame: 0 })
        .add_systems(Startup, setup_double_buffer_entities)
        .add_systems(
            Update,
            (
                increment_frame_system,
                apply_frame_1_mutations.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 1)
                    },
                ),
                verify_frame_1_buffering_isolation.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 1)
                    },
                ),
                verify_frame_2_buffered_reads.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 2)
                    },
                ),
                apply_frame_2_redundant_insert.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 2)
                    },
                ),
                verify_frame_3_decay_and_overwrites.run_if(
                    |counter: ParallelResourceAccessor<FrameCounter>| {
                        counter.scope(|c| c.current_frame == 3)
                    },
                ),
            )
                .chain(),
        );

    app.set_runner(test_runner_double_buffered);
    app.run();
}

fn test_runner_double_buffered(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
    app.update();
    app.update();
}

fn complex_added_filter_verify_frame_1(
    not_added_q: Query<&EntityTag, Not<Added<Velocity>>>,
    not_or_q: Query<&EntityTag, Not<Or<(Added<Velocity>, Added<Acceleration>)>>>,
    or_mixed_q: Query<&EntityTag, Or<(Added<Velocity>, With<Position>)>>,
) {
    let mut not_added_count = 0;
    for tag in not_added_q.iter() {
        if *tag == EntityTag::AdderTarget {
            not_added_count += 1;
        }
    }

    let mut not_or_count = 0;
    for tag in not_or_q.iter() {
        if *tag == EntityTag::AdderTarget || *tag == EntityTag::InserterTarget {
            not_or_count += 1;
        }
    }

    let mut or_mixed_count = 0;
    for tag in or_mixed_q.iter() {
        if *tag == EntityTag::AdderTarget {
            or_mixed_count += 1;
        }
    }

    assert_eq!(not_added_count, 1);
    assert_eq!(not_or_count, 2);
    assert_eq!(or_mixed_count, 1);
}

fn complex_added_filter_verify_frame_2(
    not_added_q: Query<&EntityTag, Not<Added<Velocity>>>,
    not_or_q: Query<&EntityTag, Not<Or<(Added<Velocity>, Added<Acceleration>)>>>,
    or_mixed_q: Query<&EntityTag, Or<(Added<Velocity>, With<Position>)>>,
) {
    let mut not_added_count = 0;
    for tag in not_added_q.iter() {
        if *tag == EntityTag::AdderTarget {
            not_added_count += 1;
        }
    }

    let mut not_or_count = 0;
    for tag in not_or_q.iter() {
        if *tag == EntityTag::AdderTarget || *tag == EntityTag::InserterTarget {
            not_or_count += 1;
        }
    }

    let mut or_mixed_count = 0;
    for tag in or_mixed_q.iter() {
        if *tag == EntityTag::AdderTarget {
            or_mixed_count += 1;
        }
    }

    assert_eq!(not_added_count, 0);
    assert_eq!(not_or_count, 0);
    assert_eq!(or_mixed_count, 1);
}

fn complex_added_filter_verify_frame_3(
    not_added_q: Query<&EntityTag, Not<Added<Velocity>>>,
    not_or_q: Query<&EntityTag, Not<Or<(Added<Velocity>, Added<Acceleration>)>>>,
    or_mixed_q: Query<&EntityTag, Or<(Added<Velocity>, With<Position>)>>,
) {
    let mut not_added_count = 0;
    for tag in not_added_q.iter() {
        if *tag == EntityTag::AdderTarget {
            not_added_count += 1;
        }
    }

    let mut not_or_count = 0;
    for tag in not_or_q.iter() {
        if *tag == EntityTag::AdderTarget || *tag == EntityTag::InserterTarget {
            not_or_count += 1;
        }
    }

    let mut or_mixed_count = 0;
    for tag in or_mixed_q.iter() {
        if *tag == EntityTag::AdderTarget {
            or_mixed_count += 1;
        }
    }

    assert_eq!(not_added_count, 1);
    assert_eq!(not_or_count, 2);
    assert_eq!(or_mixed_count, 1);
}

#[test_fork::test]
fn test_complex_added_combinator_filters() {
    let mut app = App::new();
    app.insert_resource(FrameCounter { current_frame: 0 })
        .add_systems(Startup, setup_double_buffer_entities);
    app.add_systems(
        Update,
        (
            increment_frame_system,
            apply_frame_1_mutations.run_if(|counter: ParallelResourceAccessor<FrameCounter>| {
                counter.scope(|c| c.current_frame == 1)
            }),
            complex_added_filter_verify_frame_1.run_if(
                |counter: ParallelResourceAccessor<FrameCounter>| {
                    counter.scope(|c| c.current_frame == 1)
                },
            ),
            complex_added_filter_verify_frame_2.run_if(
                |counter: ParallelResourceAccessor<FrameCounter>| {
                    counter.scope(|c| c.current_frame == 2)
                },
            ),
            complex_added_filter_verify_frame_3.run_if(
                |counter: ParallelResourceAccessor<FrameCounter>| {
                    counter.scope(|c| c.current_frame == 3)
                },
            ),
        )
            .chain(),
    );
    app.set_runner(test_runner_double_buffered);
    app.run();
}
