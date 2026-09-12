use avenix::prelude::*;
use std::time::Duration;

#[derive(Component)]
struct ThreadMarker(u32);

#[derive(Component)]
#[allow(dead_code)]
struct Position {
    x: f32,
    y: f32,
}

fn main() {
    println!("=== Simultaneous thread parallel commands and system injected ===\n");

    let mut app = App::new();
    app.add_plugins(DefaultSchedulesPlugin)
        .add_systems(Startup, system_parallel_spawn_system)
        .add_systems(Update, verify_parallel_spawns_system);

    app.set_runner(run_parallel_test_loop);

    let par_commands = app.world_mut().get_par_commands();

    std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(5));
            par_commands.scope(|mut cmd| {
                println!("[Thread 1] Queueing 500 entities...");
                for i in 0..500 {
                    cmd.spawn((
                        Position {
                            x: i as f32,
                            y: 1.0,
                        },
                        ThreadMarker(1),
                    ));
                }
            });
        });

        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(5));
            par_commands.scope(|mut cmd| {
                println!("[Thread 2] Queueing 500 entities...");
                for i in 0..500 {
                    cmd.spawn((
                        Position {
                            x: i as f32,
                            y: 2.0,
                        },
                        ThreadMarker(2),
                    ));
                }
            });
        });
    });

    app.run();

    println!("\n=== ParallelCommands example complete ===");
}

fn system_parallel_spawn_system(par_commands: ParallelCommands) {
    std::thread::scope(|s| {
        s.spawn(|| {
            par_commands.scope(|mut cmd| {
                println!("[System-Injected Thread 3] Queueing 500 entities...");
                for i in 0..500 {
                    cmd.spawn((
                        Position {
                            x: i as f32,
                            y: 3.0,
                        },
                        ThreadMarker(3),
                    ));
                }
            });
        });
    });
}

fn verify_parallel_spawns_system(query: Query<(&Position, &ThreadMarker)>) {
    let mut t1_count = 0;
    let mut t2_count = 0;
    let mut t3_count = 0;

    for view in query.iter() {
        for (_, marker) in view.iter() {
            if marker.0 == 1 {
                t1_count += 1;
            } else if marker.0 == 2 {
                t2_count += 1;
            } else if marker.0 == 3 {
                t3_count += 1;
            }
        }
    }

    if t1_count > 0 || t2_count > 0 || t3_count > 0 {
        println!("\n>> Avenix System Analysis:");
        println!("   -> Verified Thread 1 entities stored: {}", t1_count);
        println!("   -> Verified Thread 2 entities stored: {}", t2_count);
        println!(
            "   -> Verified System-Injected Thread 3 entities stored: {}",
            t3_count
        );

        assert_eq!(t1_count, 500);
        assert_eq!(t2_count, 500);
        assert_eq!(t3_count, 500);
        println!("   -> Success: All 1,500 parallel entities synchronized cleanly!");
    }
}

fn run_parallel_test_loop(app: &mut App) {
    app.build();
    app.run_startup();

    app.update();
    app.update();
}
