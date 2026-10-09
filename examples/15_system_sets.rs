use avenix::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GameTickStep {
    PollInput,
    DownloadNetworkPackets,
    ApplyForces,
    DetectCollisions,
    FlushPostPhysics,
    MixSpatialAudio,
    EmitParticles,
    NotifyTelemetry,
    SecondaryTrackingA,
    SecondaryTrackingB,
}

#[derive(Resource, Default)]
struct ExecutionLogger {
    history: Vec<GameTickStep>,
}

#[derive(Resource)]
struct MatchState {
    tick_paused: u32,
}

#[derive(PartialEq, Clone, SystemSet)]
struct PhysicsSet;

#[derive(PartialEq, Clone, SystemSet)]
struct NetworkingSet;

#[derive(PartialEq, Clone, SystemSet)]
struct AudioSet;

#[derive(PartialEq, Clone, SystemSet)]
struct ParticleSet;

#[derive(Clone, PartialEq, SystemSet)]
enum GameLoopStage {
    Input,
    Simulation,
    PostSim,
}

fn poll_player_input(mut logger: ResMut<ExecutionLogger>) {
    logger.history.push(GameTickStep::PollInput);
}

fn fetch_network_packets(mut logger: ResMut<ExecutionLogger>) {
    logger.history.push(GameTickStep::DownloadNetworkPackets);
}

fn apply_forces(mut logger: ResMut<ExecutionLogger>) {
    logger.history.push(GameTickStep::ApplyForces);
}

fn detect_collisions(mut logger: ResMut<ExecutionLogger>) {
    logger.history.push(GameTickStep::DetectCollisions);
}

fn draw_game_frame(mut logger: ResMut<ExecutionLogger>) {
    logger.history.push(GameTickStep::FlushPostPhysics);
}

fn play_spatial_audio(mut logger: ResMut<ExecutionLogger>) {
    logger.history.push(GameTickStep::MixSpatialAudio);
}

fn spawn_sparks(mut logger: ResMut<ExecutionLogger>) {
    logger.history.push(GameTickStep::EmitParticles);
}

fn log_telemetry_metrics(mut logger: ResMut<ExecutionLogger>) {
    logger.history.push(GameTickStep::NotifyTelemetry);
}

fn extra_telemetry_alpha(mut logger: ResMut<ExecutionLogger>) {
    logger.history.push(GameTickStep::SecondaryTrackingA);
}

fn extra_telemetry_beta(mut logger: ResMut<ExecutionLogger>) {
    logger.history.push(GameTickStep::SecondaryTrackingB);
}

fn main() {
    println!("=======================================================");
    println!("     Avenix SystemSet Architecture Evaluation Matrix    ");
    println!("=======================================================\n");

    let mut app = App::new();
    let flat_accessor = app.world_mut().get_par_resource_accessor::<MatchState>();

    app.insert_resource(ExecutionLogger::default())
        .insert_resource(MatchState { tick_paused: 0 })
        .configure_sets(
            Update,
            (
                (
                    GameLoopStage::Input,
                    GameLoopStage::Simulation.run_if(
                        move |state: &ParallelResourceAccessor<MatchState>| {
                            state.scope(|s| s.tick_paused == 0)
                        },
                    ),
                    GameLoopStage::PostSim,
                )
                    .chain(),
                (NetworkingSet, PhysicsSet).chain(),
                (PhysicsSet, NetworkingSet).in_set(GameLoopStage::Simulation),
                AudioSet.after(detect_collisions),
                ParticleSet.after(PhysicsSet),
                (AudioSet, ParticleSet).after(poll_player_input),
                GameLoopStage::PostSim.after(AudioSet).after(ParticleSet),
            ),
        )
        .add_systems(
            Update,
            (
                poll_player_input.in_set(GameLoopStage::Input),
                fetch_network_packets.in_set(NetworkingSet),
                (apply_forces, detect_collisions.after(apply_forces)).in_set(PhysicsSet),
                draw_game_frame.in_set(GameLoopStage::PostSim),
                play_spatial_audio.in_set(AudioSet),
                spawn_sparks.in_set(ParticleSet),
                log_telemetry_metrics.after(AudioSet),
                (extra_telemetry_alpha, extra_telemetry_beta)
                    .after(AudioSet)
                    .in_set(GameLoopStage::PostSim),
            ),
        );

    app.build();
    app.run_startup();

    println!("🎬 [Frame 1] Match Active (Simulation conditions satisfied):");
    app.update();
    verify_active_frame_graph(&app);
    print_logs(&mut app);

    println!("⏸️ [Frame 2] Match Paused (Simulation block skipped):");
    flat_accessor.scope_mut(|mut state| {
        state.tick_paused = 1;
    });
    app.update();
    verify_paused_frame_graph(&app);
    print_logs(&mut app);

    println!(
        " -> Matrix Evaluation Successful: Frame sorting and short-circuit optimizations verified natively!"
    );
}

fn verify_active_frame_graph(app: &App) {
    let logger = app.world().get_resource::<ExecutionLogger>();

    assert_eq!(logger.history[0], GameTickStep::PollInput);

    let net_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::DownloadNetworkPackets)
        .unwrap();
    let forces_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::ApplyForces)
        .unwrap();
    let collisions_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::DetectCollisions)
        .unwrap();
    let audio_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::MixSpatialAudio)
        .unwrap();
    let particles_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::EmitParticles)
        .unwrap();
    let telemetry_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::NotifyTelemetry)
        .unwrap();
    let tracking_a_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::SecondaryTrackingA)
        .unwrap();
    let tracking_b_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::SecondaryTrackingB)
        .unwrap();
    let flush_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::FlushPostPhysics)
        .unwrap();

    assert!(net_pos > 0);
    assert!(forces_pos > net_pos);
    assert!(collisions_pos > forces_pos);
    assert!(audio_pos > collisions_pos);
    assert!(particles_pos > forces_pos);
    assert!(telemetry_pos > audio_pos);
    assert!(tracking_a_pos > audio_pos);
    assert!(tracking_b_pos > audio_pos);
    assert!(flush_pos > audio_pos);
    assert!(flush_pos > particles_pos);
}

fn verify_paused_frame_graph(app: &App) {
    let logger = app.world().get_resource::<ExecutionLogger>();

    assert_eq!(logger.history[0], GameTickStep::PollInput);
    assert!(
        !logger
            .history
            .contains(&GameTickStep::DownloadNetworkPackets)
    );
    assert!(!logger.history.contains(&GameTickStep::ApplyForces));
    assert!(!logger.history.contains(&GameTickStep::DetectCollisions));

    let audio_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::MixSpatialAudio)
        .unwrap();
    let particles_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::EmitParticles)
        .unwrap();
    let telemetry_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::NotifyTelemetry)
        .unwrap();
    let flush_pos = logger
        .history
        .iter()
        .position(|&s| s == GameTickStep::FlushPostPhysics)
        .unwrap();

    assert!(audio_pos > 0);
    assert!(particles_pos > 0);
    assert!(telemetry_pos > audio_pos);
    assert!(flush_pos > audio_pos);
    assert!(flush_pos > particles_pos);
}

fn print_logs(app: &mut App) {
    let mut logger = app.world().get_resource_mut::<ExecutionLogger>();
    for trace in logger.history.drain(..) {
        println!("{:?}", trace);
    }
    println!();
}
