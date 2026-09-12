use avenix::prelude::*;

#[derive(Component)]
struct PlayerTag;
#[derive(Component)]
struct EnemyTag;
#[derive(Component)]
struct BossTag;
#[derive(Component)]
struct TargetEntity(Entity);
#[derive(Component)]
struct Health(i32);

fn main() {
    println!("=== Random Query lookups example ===\n");

    let mut app = App::new();
    app.add_plugins(DefaultSchedulesPlugin)
        .add_systems(Startup, setup_simulation_entities_system)
        .add_systems(
            Update,
            (
                simple_global_lookup_system,
                complex_coordinated_view_lookup_system,
                zero_allocation_nested_lookup_system,
            ),
        );

    app.set_runner(run_lookup_test_loop);
    app.run();

    println!("\n=== Example completed ===");
}

fn setup_simulation_entities_system(mut commands: Commands) {
    println!("[Startup] Setting up simulation entities...");
    commands.spawn((Health(100), PlayerTag));
    commands.spawn((Health(500), EnemyTag));
    commands.spawn((Health(1000), BossTag));
}

fn simple_global_lookup_system(
    mut query_health: Query<&mut Health>,
    query_player: Query<(Entity, &PlayerTag)>,
) {
    for view in query_player.iter() {
        for (entity, _tag) in view.iter() {
            #[allow(unused_mut)]
            if let Some(mut health) = query_health.get_mut(entity) {
                if health.0 == 100 {
                    println!("\n[Simple Lookup] Executing Query-level O(1) fetch:");
                    println!("   -> Located Player Entity via view extraction loop.");
                    println!("   -> Initial Player health: {}", health.0);
                    health.0 -= 20;
                    println!("   -> Player health cleanly updated to: {}", health.0);
                    assert_eq!(health.0, 80);
                }
            }
        }
    }
}

fn complex_coordinated_view_lookup_system(
    mut query_health: Query<&mut Health>,
    query_enemy: Query<(Entity, &EnemyTag)>,
) {
    let mut targets_to_process = Vec::new();

    for view in query_enemy.iter() {
        for (entity, _tag) in view.iter() {
            targets_to_process.push(TargetEntity(entity.clone()));
        }
    }

    for mut view in query_health.iter_mut() {
        for target in &targets_to_process {
            #[allow(unused_mut)]
            if let Some(mut health) = view.get_mut(&target.0) {
                if health.0 == 500 {
                    println!(
                        "\n[Complex Coordinated Lookup] Executing vector-staged View-level array fetch:"
                    );
                    println!(
                        "   -> Pulling vector targets directly from active Archetype View matrix."
                    );
                    println!("   -> Initial Enemy health: {}", health.0);
                    health.0 -= 100;
                    println!("   -> Enemy health cleanly modified to: {}", health.0);
                    assert_eq!(health.0, 400);
                }
            }
        }
    }
}

fn zero_allocation_nested_lookup_system(
    mut query_health: Query<&mut Health>,
    query_boss: Query<(Entity, &BossTag)>,
) {
    for boss_view in query_boss.iter() {
        for (entity, _tag) in boss_view.iter() {
            for mut health_view in query_health.iter_mut() {
                #[allow(unused_mut)]
                if let Some(mut health) = health_view.get_mut(entity) {
                    #[allow(unused_mut)]
                    if health.0 == 1000 {
                        println!(
                            "\n[Zero-Allocation Nested Lookup] Executing zero-heap nested array match:"
                        );
                        println!(
                            "   -> Completely bypassed staging vectors; streaming directly across views."
                        );
                        println!("   -> Initial Boss health: {}", health.0);
                        health.0 -= 200;
                        println!("   -> Boss health cleanly modified to: {}", health.0);
                        assert_eq!(health.0, 800);
                    }
                }
            }
        }
    }
}

fn run_lookup_test_loop(app: &mut App) {
    app.build();
    app.run_startup();

    app.update();
    app.update();
}
