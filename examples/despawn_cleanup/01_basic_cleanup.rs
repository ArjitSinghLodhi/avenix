use avenix::prelude::*;

#[derive(Resource)]
pub struct ActiveTarget {
    pub entity: Option<Entity>,
}

#[derive(Component)]
#[component(on_remove = Enemy::handle_despawn)]
struct Enemy;

impl Enemy {
    fn handle_despawn(world: &mut ExclusiveWorld, entity: &Entity) {
        if let Some(mut target) = world.get_resource_mut::<ActiveTarget>() {
            if target.entity.as_ref() == Some(entity) {
                println!(
                    "Cleanup Hook: Intercepted hardware despawn for active target. Evicting leaked handle instantly."
                );
                target.entity = None;
            }
        }
    }
}

fn spawn_initial_entities(commands: Commands) {
    println!("Startup Step 1: Spawning target entity...");
    commands.spawn(Enemy);
}

fn track_initial_entities(query: Query<Entity, With<Enemy>>, mut target: ResMut<ActiveTarget>) {
    if let Some(enemy_entity) = query.iter().next() {
        println!("Startup Step 2: Extracting flushed Entity handle from Query.");
        target.entity = Some(enemy_entity.clone());
    }
}

fn trigger_runtime_despawn(commands: Commands, query: Query<Entity, With<Enemy>>) {
    if let Some(enemy_entity) = query.iter().next() {
        println!("Update Phase: Core logic requested hard hardware despawn of target entity!");
        commands.entity(enemy_entity).despawn();
    }
}

fn verify_handle_state(target: &ActiveTarget) {
    match target.entity {
        Some(_) => println!("Verification Phase: Target handle remains active."),
        None => println!(
            "Verification Phase: Handle was successfully intercepted and removed! Invariant satisfied."
        ),
    }
}

fn run_simulation_flow(app: &mut App) {
    app.build();
    app.run_startup();

    println!("\n--- Step 1: Active Tracking State After Startup ---");
    verify_handle_state(&app.world().resource::<ActiveTarget>());

    println!("\n--- Step 2: Trigger Despawn and Run Hook ---");
    app.update();

    println!("\n--- Step 3: Post-Execution Verification ---");
    verify_handle_state(&app.world().resource::<ActiveTarget>());
}

fn main() {
    println!("--- Starting Basic Cleanup Example ---");

    App::new()
        .insert_resource(ActiveTarget { entity: None })
        .add_systems(
            Startup,
            (spawn_initial_entities, track_initial_entities).chain(),
        )
        .add_systems(Update, trigger_runtime_despawn)
        .set_runner(run_simulation_flow)
        .run();
}
