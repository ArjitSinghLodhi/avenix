use avenix::prelude::*;
use std::{
    sync::{Arc, Barrier},
    time::Duration,
};

#[derive(Component)]
struct Velocity {
    x: f32,
    y: f32,
}

#[derive(Component)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Component)]
struct IsFollower;

#[derive(Resource, Clone)]
struct TestSyncContext {
    barrier: Arc<Barrier>,
}

#[test_fork::test]
fn test_parallel_query_accessor_mixed_workload() {
    let mut app = App::new();

    let par_reader = app.get_par_query_accessor::<(&Position, &Velocity), EmptyQueryFilter>();
    let par_writer = app.get_par_query_accessor::<(&mut Position, &Velocity), EmptyQueryFilter>();
    let par_reactive = app.get_par_query_accessor::<&Position, Changed<Position>>();

    app.add_systems(Startup, setup_simulation_entities)
        .add_systems(Update, multi_query_in_band_system);

    let shared_barrier = Arc::new(Barrier::new(3));
    app.insert_resource(TestSyncContext {
        barrier: shared_barrier.clone(),
    });

    app.build();
    app.run_startup();

    std::thread::scope(|s| {
        let b1 = shared_barrier.clone();
        let b2 = shared_barrier.clone();

        s.spawn(move || {
            b1.wait();

            par_reader.scope(|query| {
                let mut read_count = 0;
                for view in query.iter() {
                    for (pos, vel) in view.iter() {
                        read_count += 1;
                        assert_eq!(vel.x, 2.0);
                        assert!(pos.x >= 0.0);
                    }
                }
                assert_eq!(read_count, 1000);
            });
        });

        s.spawn(move || {
            b2.wait();
            std::thread::sleep(Duration::from_millis(2));

            par_writer.scope(|mut query| {
                for mut view in query.iter_mut() {
                    for (mut pos, vel) in view.iter_mut() {
                        pos.x += vel.x;
                        pos.y += vel.y;
                    }
                }
            });
        });

        app.update();
    });

    par_reactive.scope(|query| {
        let mut total_reactive_catch = 0;
        for view in query.iter() {
            total_reactive_catch += view.len();
        }
        assert_eq!(total_reactive_catch, 500);
    });
}

fn setup_simulation_entities(commands: Commands) {
    let batch_leaders = (0..500).map(|i| {
        (
            Position {
                x: i as f32,
                y: 0.0,
            },
            Velocity { x: 2.0, y: 2.0 },
        )
    });
    commands.spawn_batch(batch_leaders);

    let batch_followers = (0..500).map(|i| {
        (
            Position {
                x: i as f32,
                y: 0.0,
            },
            Velocity { x: 2.0, y: 2.0 },
            IsFollower,
        )
    });
    commands.spawn_batch(batch_followers);
}

fn multi_query_in_band_system(
    mut query_write: Query<(&mut Position, &Velocity), Without<IsFollower>>,
    query_read: Query<&Position, With<IsFollower>>,
    sync_ctx: Res<TestSyncContext>,
) {
    sync_ctx.barrier.wait();

    let mut system_read_count = 0;
    for view in query_read.iter() {
        system_read_count += view.len();
    }
    assert_eq!(system_read_count, 500);

    for mut view in query_write.iter_mut() {
        for (mut pos, vel) in view.iter_mut() {
            pos.x += vel.x * 0.1;
        }
    }
}

#[test_fork::test]
fn test_parallel_query_accessor_heavy_mutation_chaos() {
    let mut app = App::new();

    let par_reader = app.get_par_query_accessor::<(&Position, &Velocity), EmptyQueryFilter>();
    let par_writer = app.get_par_query_accessor::<(&mut Position, &Velocity), EmptyQueryFilter>();

    app.add_systems(Startup, setup_simulation_entities)
        .add_systems(Update, dynamic_chaos_mutator_system);

    let shared_barrier = Arc::new(Barrier::new(3));
    app.insert_resource(TestSyncContext {
        barrier: shared_barrier.clone(),
    });

    app.build();
    app.run_startup();

    std::thread::scope(|s| {
        let b1 = shared_barrier.clone();
        let b2 = shared_barrier.clone();

        s.spawn(move || {
            b1.wait();
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_millis(50) {
                par_reader.scope(|query| {
                    let mut count = 0;
                    for view in query.iter() {
                        count += view.len();
                        for (pos, vel) in view.iter() {
                            assert!(pos.x >= 0.0 || pos.x < 0.0);
                            assert_eq!(vel.x, 2.0);
                        }
                    }
                    assert!(count <= 1000);
                });
                std::thread::yield_now();
            }
        });

        s.spawn(move || {
            b2.wait();
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_millis(50) {
                par_writer.scope(|mut query| {
                    for mut view in query.iter_mut() {
                        for (mut pos, vel) in view.iter_mut() {
                            pos.x += vel.x * 0.01;
                        }
                    }
                });
                std::thread::yield_now();
            }
        });

        app.update();
    });
}

fn dynamic_chaos_mutator_system(
    query: Query<Entity, Without<IsFollower>>,
    commands: Commands,
    sync_ctx: Res<TestSyncContext>,
) {
    sync_ctx.barrier.wait();
    let mut i = 0;
    for view in query.iter() {
        for entity in view.iter() {
            if i % 2 == 0 {
                commands.entity(entity.clone()).add(IsFollower);
            } else {
                commands.entity(entity.clone()).remove::<Velocity>();
            }
            i += 1;
        }
    }
}
