use avenix::prelude::*;

#[derive(Resource, Debug, PartialEq)]
struct GlobalScore {
    value: u32,
}

#[derive(Resource, Debug, PartialEq)]
struct EngineConfig {
    max_fps: u32,
}

#[derive(Resource, Debug, PartialEq)]
struct OptionalFeature {
    enabled: bool,
}

#[derive(Resource)]
struct FrameCounter {
    current_frame: u32,
}

fn main() {
    println!("=== Simultaneous Thread Parallel Resources Example ===\n");

    let mut app = App::new();

    app.insert_resource(FrameCounter { current_frame: 0 })
        .insert_resource(GlobalScore { value: 100 })
        .insert_resource(EngineConfig { max_fps: 60 });

    app.add_systems(
        Update,
        (increment_frame_system, parallel_verification_system),
    );
    app.set_runner(run_resource_test_loop);

    let score_accessor = app.world_mut().get_par_resource_accessor::<GlobalScore>();
    let config_accessor = app.world_mut().get_par_resource_accessor::<EngineConfig>();
    let feature_accessor = app
        .world_mut()
        .get_par_resource_accessor::<OptionalFeature>();

    std::thread::scope(|s| {
        s.spawn(|| {
            score_accessor.scope(|score| {
                println!(
                    "[Thread 1] Read shared baseline GlobalScore: {}",
                    score.value
                );
                assert_eq!(score.value, 100);
            });
        });

        s.spawn(|| {
            println!("[Thread 1] Updating EngineConfig max_fps to 120...");
            config_accessor.scope_mut(|mut config| {
                config.max_fps = 120;
            });
        });

        s.spawn(|| {
            println!("[Thread 2] Checking if OptionalFeature exists initially...");
            assert!(!feature_accessor.is_present());

            feature_accessor.scope_opt(|opt| {
                assert!(opt.is_none());
                println!(" -> Verified: scope_opt safely returns None if missing.");
            });
            feature_accessor.scope_mut_opt(|opt| {
                assert!(opt.is_none());
                println!(" -> Verified: scope_mut_opt safely returns None if missing.");
            });

            println!("[Thread 2] Dynamically injecting OptionalFeature...");
            let old_feature = feature_accessor.insert_resource(OptionalFeature { enabled: true });
            assert!(old_feature.is_none());

            assert!(feature_accessor.is_present());

            feature_accessor.scope_mut_opt(|mut opt| {
                let res = opt.as_mut().unwrap();
                res.enabled = false;
            });
        });
    });

    app.run();

    println!("\n=== Parallel Resources Example Complete ===");
}

fn increment_frame_system(mut tracker: ResMut<FrameCounter>) {
    tracker.current_frame += 1;
}

fn parallel_verification_system(
    counter: Res<FrameCounter>,
    score_accessor: ParallelResourceAccessor<GlobalScore>,
    config_accessor: ParallelResourceAccessor<EngineConfig>,
    feature_accessor: ParallelResourceAccessor<OptionalFeature>,
) {
    if counter.current_frame == 1 {
        println!("\n>> Avenix System Injected Verification Analysis (Frame 1):");

        config_accessor.scope(|config| {
            println!(
                " -> Verified EngineConfig mutation: max_fps = {}",
                config.max_fps
            );
            assert_eq!(config.max_fps, 120);
        });

        feature_accessor.scope(|feature| {
            println!(
                " -> Verified Dynamic OptionalFeature mutation: enabled = {}",
                feature.enabled
            );
            assert_eq!(feature.enabled, false);
        });

        std::thread::scope(|s| {
            s.spawn(|| {
                score_accessor.scope_mut(|mut score| {
                    println!("[System-Spawned Thread 3] Incrementing GlobalScore (+50)...");
                    score.value += 50;
                });
            });
        });
    } else if counter.current_frame == 2 {
        println!("\n>> Avenix System Injected Verification Analysis (Frame 2):");

        score_accessor.scope(|score| {
            println!(
                " -> Verified GlobalScore accumulation from Thread 3: {}",
                score.value
            );
            assert_eq!(score.value, 150);
        });

        println!(" -> Attempting dynamic teardown of GlobalScore...");
        let removed_score = score_accessor.remove_resource();
        assert!(removed_score.is_some());
        assert_eq!(removed_score.unwrap().value, 150);

        assert!(!score_accessor.is_present());
        println!(" -> Success: GlobalScore successfully popped out of core registry safely!");
    }
}

fn run_resource_test_loop(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
    app.update();
}
