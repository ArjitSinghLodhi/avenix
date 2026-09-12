use avenix::prelude::*;

#[derive(Component)]
#[allow(dead_code)]
struct ThreadMarker(u32);

#[derive(Component)]
#[allow(dead_code)]
struct Position {
    x: f32,
    y: f32,
}
#[derive(Component)]
struct Velocity {
    x: f32,
    y: f32,
}

fn main() {
    println!("=== Sequential quries example ===\n");

    let mut app = App::new();
    app.add_plugins(DefaultSchedulesPlugin)
        .add_systems(Startup, spawn_entities_system)
        .add_systems(Update, (sequential_read_system, sequential_write_system));

    app.set_runner(run_query_test_loop);
    app.run();

    println!("\n=== Example completed ===");
}

fn spawn_entities_system(mut commands: Commands) {
    println!("Spawning entities via fast batch allocation at startup...");
    let batch = (0..1000).map(|i| {
        (
            Position {
                x: i as f32,
                y: 0.0,
            },
            Velocity { x: 1.0, y: 1.0 },
            ThreadMarker(0),
        )
    });
    commands.spawn_batch(batch);
}

fn sequential_read_system(query: Query<(&Position, &Velocity)>) {
    let mut total_entities = 0;

    for view in query.iter() {
        for (pos, _vel) in view.iter() {
            total_entities += 1;
            if total_entities == 1 {
                println!(
                    "[Sequential Read] First Entity Position: ({}, {})",
                    pos.x, pos.y
                );
            }
        }
    }
}

fn sequential_write_system(mut query: Query<(&mut Position, &Velocity)>) {
    let start = std::time::Instant::now();
    let mut count = 0;

    for mut view in query.iter_mut() {
        #[allow(unused_mut)]
        for (mut pos, vel) in view.iter_mut() {
            pos.x += vel.x;
            pos.y += vel.y;
            count += 1;
        }
    }

    if count > 0 {
        println!(
            "[Sequential Write] Updated {} entities sequentially in {:?}",
            count,
            start.elapsed()
        );
    }
}

fn run_query_test_loop(app: &mut App) {
    app.build();
    app.run_startup();

    app.update();
    app.update();
}
