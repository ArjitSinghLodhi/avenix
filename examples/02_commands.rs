use avenix::prelude::*;
fn test_runner_once(app: &mut App) {
    app.build();
    app.run_startup();

    println!("--- Frame 1 ---");
    app.update();
    println!("--- Frame 2 ---");
    app.update();
}

#[derive(Component)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Component)]
struct Speed(f32);

fn main() {
    App::new()
        .add_plugins(DefaultSchedulesPlugin)
        .add_systems(Startup, queue_spawn_commands_system)
        .add_systems(Update, verify_spawned_entities_system)
        .set_runner(test_runner_once)
        .run();
}

fn queue_spawn_commands_system(mut commands: Commands) {
    println!("Queueing a single entity spawn via a component tuple...");
    commands.spawn((Position { x: 0.0, y: 0.0 }, Speed(5.0)));

    println!("Queueing a batch spawn of 1,000 entities via tuple iterator...");
    let batch = (0..1000).map(|i| {
        (
            Position {
                x: i as f32,
                y: 10.0,
            },
            Speed(1.0),
        )
    });
    commands.spawn_batch(batch);
}

fn verify_spawned_entities_system(query: Query<(&Position, &Speed)>) {
    let mut count = 0;
    for view in query.iter() {
        for (pos, speed) in view.iter() {
            count += 1;
            if count == 1 || count == 1001 {
                println!(
                    "Inspecting Entity: Position({}, {}), Speed: {}",
                    pos.x, pos.y, speed.0
                );
            }
        }
    }
    println!(
        "Successfully verified {} entities spawned via Commands!",
        count
    );
}
