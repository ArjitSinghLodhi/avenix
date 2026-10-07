use avenix::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Resource, Default)]
struct ExecutionLogger {
    history: Vec<GameTickStep>,
}

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

#[derive(Resource, Clone)]
struct MatchState {
    tick_paused: Arc<AtomicU32>,
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
    println!("     Avenix Engine: Advanced SystemSet Architecture     ");
    println!("=======================================================\n");

    let mut app = App::new();
    let pause_flag = Arc::new(AtomicU32::new(0));

    app.insert_resource(ExecutionLogger::default())
        .insert_resource(MatchState {
            tick_paused: pause_flag.clone(),
        })
        .configure_sets(
            Update,
            (
                GameLoopStage::Input,
                GameLoopStage::Simulation
                    .after(GameLoopStage::Input)
                    .run_if(move |state: &ParallelResourceAccessor<MatchState>| {
                        state.scope(|s| s.tick_paused.load(Ordering::Relaxed) == 0)
                    }),
                GameLoopStage::PostSim.after(GameLoopStage::Simulation),
                (PhysicsSet, NetworkingSet).in_set(GameLoopStage::Simulation),
                NetworkingSet.before(PhysicsSet),
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
                apply_forces.in_set(PhysicsSet),
                detect_collisions.in_set(PhysicsSet).after(apply_forces),
                draw_game_frame.in_set(GameLoopStage::PostSim),
                play_spatial_audio.in_set(AudioSet),
                spawn_sparks.in_set(ParticleSet),
                log_telemetry_metrics.after(AudioSet),
            ),
        )
        .add_systems(
            Update,
            (extra_telemetry_alpha, extra_telemetry_beta)
                .after(AudioSet)
                .in_set(GameLoopStage::PostSim),
        );

    app.build();
    app.run_startup();

    println!("🎬 [Frame 1] Match Active (Simulation conditions satisfied):");
    app.update();
    print_logs(&mut app);

    println!("⏸️ [Frame 2] Match Paused (Simulation block skipped):");
    pause_flag.store(1, Ordering::Relaxed);
    app.update();
    print_logs(&mut app);
}

fn print_logs(app: &mut App) {
    let mut logger = app.world().get_resource_mut::<ExecutionLogger>();
    for trace in logger.history.drain(..) {
        println!("{:?}", trace);
    }
    println!();
}
