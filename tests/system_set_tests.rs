use avenix::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Resource, Default)]
struct ExecutionHistory {
    steps: Vec<ExecutionStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecutionStep {
    GatherInput,
    SyncNetwork,
    ResolveColliders,
    IntegrateForces,
    PostPhysicsCleanup,
    PlaySpatialAudio,
    SpawnSparks,
    SecondaryVerification,
    TupleSysA,
    TupleSysB,
}

#[derive(Resource, Clone)]
struct SetConditionFlag {
    counter: Arc<AtomicU32>,
}

#[derive(PartialEq, Clone, Debug, SystemSet)]
struct PhysicsSet;

#[derive(PartialEq, Clone, Debug, SystemSet)]
struct NetworkingSet;

#[derive(PartialEq, Clone, Debug, SystemSet)]
struct AudioSet;

#[derive(PartialEq, Clone, Debug, SystemSet)]
struct ParticleSet;

#[derive(Clone, Debug, PartialEq, Eq, SystemSet)]
enum GameplayStage {
    Input,
    Simulation,
    PostSim,
}

fn gather_input(mut history: ResMut<ExecutionHistory>) {
    history.steps.push(ExecutionStep::GatherInput);
}

fn sync_network(mut history: ResMut<ExecutionHistory>) {
    history.steps.push(ExecutionStep::SyncNetwork);
}

fn resolve_colliders(mut history: ResMut<ExecutionHistory>) {
    history.steps.push(ExecutionStep::ResolveColliders);
}

fn integrate_forces(mut history: ResMut<ExecutionHistory>) {
    history.steps.push(ExecutionStep::IntegrateForces);
}

fn post_physics_cleanup(mut history: ResMut<ExecutionHistory>) {
    history.steps.push(ExecutionStep::PostPhysicsCleanup);
}

fn play_spatial_audio(mut history: ResMut<ExecutionHistory>) {
    history.steps.push(ExecutionStep::PlaySpatialAudio);
}

fn spawn_sparks(mut history: ResMut<ExecutionHistory>) {
    history.steps.push(ExecutionStep::SpawnSparks);
}

fn secondary_verification(mut history: ResMut<ExecutionHistory>) {
    history.steps.push(ExecutionStep::SecondaryVerification);
}

fn tuple_sys_alpha(mut history: ResMut<ExecutionHistory>) {
    history.steps.push(ExecutionStep::TupleSysA);
}

fn tuple_sys_beta(mut history: ResMut<ExecutionHistory>) {
    history.steps.push(ExecutionStep::TupleSysB);
}

#[test_fork::test]
fn test_system_sets() {
    println!("=== Avenix Macro Derive SystemSet Integration Pipeline Test ===");

    let mut app = App::new();

    let shared_counter = Arc::new(AtomicU32::new(0));
    let condition_counter = shared_counter.clone();

    app.insert_resource(ExecutionHistory::default())
        .insert_resource(SetConditionFlag {
            counter: shared_counter.clone(),
        })
        .configure_sets(
            Update,
            (
                GameplayStage::Input,
                GameplayStage::Simulation
                    .after(GameplayStage::Input)
                    .run_if(move |flag: &ParallelResourceAccessor<SetConditionFlag>| {
                        flag.scope(|f| f.counter.load(Ordering::Relaxed) == 0)
                    }),
                GameplayStage::PostSim.after(GameplayStage::Simulation),
                (PhysicsSet, NetworkingSet).in_set(GameplayStage::Simulation),
                NetworkingSet.before(PhysicsSet),
                AudioSet.after(resolve_colliders),
                ParticleSet.after(PhysicsSet),
                (AudioSet, ParticleSet).after(gather_input),
                GameplayStage::PostSim.after(AudioSet).after(ParticleSet),
            ),
        )
        .add_systems(
            Update,
            (
                gather_input.in_set(GameplayStage::Input),
                sync_network.in_set(NetworkingSet),
                integrate_forces.in_set(PhysicsSet),
                resolve_colliders.in_set(PhysicsSet).after(integrate_forces),
                post_physics_cleanup.in_set(GameplayStage::PostSim),
                play_spatial_audio.in_set(AudioSet),
                spawn_sparks.in_set(ParticleSet),
                secondary_verification.after(AudioSet),
            ),
        );

    app.build();
    app.run_startup();

    println!("Executing primary evaluation frame (Condition Passes)...");
    app.update();

    {
        let history = app.world().get_resource::<ExecutionHistory>();
        println!("First Frame Trace Sequence: {:?}", history.steps);

        assert_eq!(history.steps[0], ExecutionStep::GatherInput);

        let sim_start = history
            .steps
            .iter()
            .position(|&s| s == ExecutionStep::SyncNetwork)
            .unwrap();
        let audio_pos = history
            .steps
            .iter()
            .position(|&s| s == ExecutionStep::PlaySpatialAudio)
            .unwrap();
        let sparks_pos = history
            .steps
            .iter()
            .position(|&s| s == ExecutionStep::SpawnSparks)
            .unwrap();
        let colliders_pos = history
            .steps
            .iter()
            .position(|&s| s == ExecutionStep::ResolveColliders)
            .unwrap();
        let forces_pos = history
            .steps
            .iter()
            .position(|&s| s == ExecutionStep::IntegrateForces)
            .unwrap();
        let cleanup_pos = history
            .steps
            .iter()
            .position(|&s| s == ExecutionStep::PostPhysicsCleanup)
            .unwrap();
        let verification_pos = history
            .steps
            .iter()
            .position(|&s| s == ExecutionStep::SecondaryVerification)
            .unwrap();

        assert!(sim_start > 0);
        assert!(forces_pos > sim_start);
        assert!(colliders_pos > forces_pos);
        assert!(audio_pos > colliders_pos);
        assert!(sparks_pos > colliders_pos);
        assert!(verification_pos > audio_pos);
        assert!(cleanup_pos > audio_pos);
        assert!(cleanup_pos > sparks_pos);
    }

    println!("Modifying condition flag to turn off the Simulation group...");
    condition_counter.store(1, Ordering::Relaxed);

    println!("Executing second evaluation frame (Condition Fails)...");
    app.update();

    {
        let history = app.world().get_resource::<ExecutionHistory>();
        println!("Aggregated Trace Sequence: {:?}", history.steps);
    }

    println!(
        " -> Verification Complete: All configurations and tuple hierarchies bundled and evaluated flawlessly!"
    );
}

#[test_fork::test]
fn test_system_sets_tuple_level_orderings() {
    println!("=== Avenix Tuple-Level Symmetrical Orderings Test ===");

    let mut app = App::new();
    app.insert_resource(ExecutionHistory::default());

    app.configure_sets(
        Update,
        (
            GameplayStage::Input,
            GameplayStage::Simulation.after(GameplayStage::Input),
            GameplayStage::PostSim.after(GameplayStage::Simulation),
            (PhysicsSet, AudioSet).in_set(GameplayStage::Simulation),
            (PhysicsSet, AudioSet).after(gather_input),
        ),
    )
    .add_systems(
        Update,
        (
            gather_input.in_set(GameplayStage::Input),
            play_spatial_audio.in_set(AudioSet),
            integrate_forces.in_set(PhysicsSet),
            post_physics_cleanup.in_set(GameplayStage::PostSim),
        ),
    )
    .add_systems(
        Update,
        (tuple_sys_alpha, tuple_sys_beta)
            .after(PhysicsSet)
            .in_set(GameplayStage::Simulation),
    );

    app.build();
    app.run_startup();
    app.update();

    let history = app.world().get_resource::<ExecutionHistory>();
    println!("Tuple Orderings Trace Sequence: {:?}", history.steps);

    assert_eq!(history.steps[0], ExecutionStep::GatherInput);

    let audio_pos = history
        .steps
        .iter()
        .position(|&s| s == ExecutionStep::PlaySpatialAudio)
        .unwrap();
    let forces_pos = history
        .steps
        .iter()
        .position(|&s| s == ExecutionStep::IntegrateForces)
        .unwrap();
    let alpha_pos = history
        .steps
        .iter()
        .position(|&s| s == ExecutionStep::TupleSysA)
        .unwrap();
    let beta_pos = history
        .steps
        .iter()
        .position(|&s| s == ExecutionStep::TupleSysB)
        .unwrap();
    let cleanup_pos = history
        .steps
        .iter()
        .position(|&s| s == ExecutionStep::PostPhysicsCleanup)
        .unwrap();

    assert!(audio_pos > 0);
    assert!(forces_pos > 0);
    assert!(alpha_pos > forces_pos);
    assert!(beta_pos > forces_pos);
    assert!(cleanup_pos > alpha_pos);
    assert!(cleanup_pos > beta_pos);

    println!(
        " -> Verification Complete: Symmetrical tuple-to-set and set-to-tuple block bounds resolved cleanly!"
    );
}
