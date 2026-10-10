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
    app.add_systems(Startup, setup_simulation_entities_system)
        .add_systems(
            Update,
            (
                simple_global_lookup_system,
                allocation_lookup_system,
                zero_allocation_nested_lookup_system,
            )
                .chain(),
        );

    app.set_runner(run_lookup_test_loop);
    app.run();

    println!("\n=== Example completed ===");
}

fn setup_simulation_entities_system(commands: Commands) {
    commands.spawn((Health(100), PlayerTag));
    commands.spawn((Health(500), EnemyTag));
    commands.spawn((Health(1000), BossTag));
}

fn simple_global_lookup_system(
    mut query_health: Query<&mut Health>,
    query_player: Query<Entity, With<PlayerTag>>,
) {
    for entity in query_player.iter() {
        #[allow(unused_mut)]
        if let Some(mut health) = query_health.get_mut(entity) {
            if health.0 == 100 {
                health.0 -= 20;
                assert_eq!(health.0, 80);
            }
        }
    }
}

fn allocation_lookup_system(
    mut query_health: Query<&mut Health>,
    query_enemy: Query<Entity, With<EnemyTag>>,
) {
    let mut targets_to_process = Vec::new();

    for entity in query_enemy.iter() {
        targets_to_process.push(TargetEntity(entity.clone()));
    }

    for target in &targets_to_process {
        #[allow(unused_mut)]
        if let Some(mut health) = query_health.get_mut(&target.0) {
            if health.0 == 500 {
                health.0 -= 100;
                assert_eq!(health.0, 400);
            }
        }
    }
}

fn zero_allocation_nested_lookup_system(
    mut query_health: Query<&mut Health>,
    query_boss: Query<Entity, With<BossTag>>,
) {
    for entity in query_boss.iter() {
        #[allow(unused_mut)]
        if let Some(mut health) = query_health.get_mut(&entity) {
            if health.0 == 1000 {
                health.0 -= 200;
                assert_eq!(health.0, 800);
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
