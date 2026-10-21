use avenix::prelude::*;

#[derive(Resource)]
pub struct ActiveTarget {
    pub entity: Option<Entity>,
}

#[derive(Component)]
struct Enemy;

pub struct BasicCleanupPlugin;

impl Plugin for BasicCleanupPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ActiveTarget { entity: None })
            .add_systems(CleanupHandles, clear_dead_target_handle);
    }
}

fn clear_dead_target_handle(commands: Commands, mut target: ResMut<ActiveTarget>) {
    commands.for_each_despawn(|despawned_entity| {
        if target.entity.as_ref() == Some(despawned_entity) {
            println!("Cleanup Gate: Intercepted hardware despawn for active target. Evicting leaked handle to prevent diagnostic crash.");
            target.entity = None;
        }
    });
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

    println!("\n--- Step 2: Trigger Despawn and Flush Cleanup Gate ---");
    app.update();

    println!("\n--- Step 3: Post-Execution Verification ---");
    verify_handle_state(&app.world().resource::<ActiveTarget>());
}

fn main() {
    println!("--- Starting Basic Cleanup Example ---");

    App::new()
        .add_plugins(BasicCleanupPlugin)
        .add_systems(
            Startup,
            (spawn_initial_entities, track_initial_entities).chain(),
        )
        .add_systems(Update, trigger_runtime_despawn)
        .set_runner(run_simulation_flow)
        .run();
}
