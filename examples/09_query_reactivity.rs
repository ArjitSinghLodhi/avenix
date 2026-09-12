use avenix::prelude::*;
use std::thread::sleep;
use std::time::Duration;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Score(u32);

#[derive(Component)]
struct Buff {
    name: &'static str,
}

#[derive(Component)]
struct StatusEffect;

#[derive(Resource)]
struct GameLoopCounter {
    frame: u32,
}

fn game_loop_driver(mut counter: ResMut<GameLoopCounter>) {
    counter.frame += 1;
    println!("\n--- ⏳ [FRAME {}] ---", counter.frame);
}

fn setup_game(mut commands: Commands) {
    println!("🚀 [Setup/Frame 0] Spawning player entity with a baseline Score.");
    commands.spawn((Player, Score(0)));
}

fn simulate_gameplay_mutations(
    counter: Res<GameLoopCounter>,
    mut commands: Commands,
    mut score_query: Query<(Entity, &mut Score)>,
) {
    if counter.frame == 1 {
        for view in score_query.iter() {
            for (entity, _) in view.iter() {
                println!("✨ [Mutation - Frame 1] commands.add_components() queued for 'Buff'.");
                commands.add_components(entity.clone(), Buff { name: "Haste" });
            }
        }
    }

    if counter.frame == 2 {
        for mut view in score_query.iter_mut() {
            for (entity, mut score) in view.iter_mut() {
                println!(
                    "🎯 [Mutation - Frame 2] Direct Mut Write modifying Score component value."
                );
                score.0 = 100;

                println!(
                    "⚡ [Mutation - Frame 2] commands.insert_components() queued for 'StatusEffect'."
                );
                commands.insert_components(entity.clone(), StatusEffect);
            }
        }
    }
}

fn reactive_added_filter_system(
    _counter: Res<GameLoopCounter>,
    added_buffs: Query<&Buff, Added<Buff>>,
    added_effects: Query<&StatusEffect, Added<StatusEffect>>,
) {
    for view in added_buffs.iter() {
        for buff in view.iter() {
            println!(
                "📥 [Reactive Filter] Added<Buff> caught structural addition: '{}'",
                buff.name
            );
        }
    }

    for view in added_effects.iter() {
        for _ in view.iter() {
            println!(
                "📥 [Reactive Filter] Added<StatusEffect> caught secondary insert structural split!"
            );
        }
    }
}

fn reactive_changed_filter_system(
    counter: Res<GameLoopCounter>,
    changed_scores: Query<&Score, Changed<Score>>,
) {
    for view in changed_scores.iter() {
        for score in view.iter() {
            println!(
                "🔥 [Reactive Filter] Changed<Score> filter caught historical mutation! Value: {}",
                score.0
            );
        }
    }

    if counter.frame == 4 {
        let count = changed_scores.iter().map(|v| v.len()).sum::<usize>();
        println!(
            "🍃 [Lifecycle Decay] Frame 4 Changed<Score> item count: {} (State safely decayed to normal).",
            count
        );
    }
}

fn granular_tracker_inspection_system(
    counter: Res<GameLoopCounter>,
    tracker_query: Query<(&Score, ChangedTracker<Score>)>,
) {
    for view in tracker_query.iter() {
        for (score, tracker) in view.iter() {
            if tracker.is_changed() {
                println!(
                    "🔍 [Tracker Inspect] Frame {} - ChangedTracker detected entry delta without narrowing query array. (Score: {})",
                    counter.frame, score.0
                );
            }
        }
    }
}

fn test_runner_four_frames(app: &mut App) {
    app.build();
    app.run_startup();

    for _ in 0..4 {
        app.update();
        sleep(Duration::from_millis(50));
    }
}

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultSchedulesPlugin)
        .insert_resource(GameLoopCounter { frame: 0 })
        .add_systems(Startup, setup_game);

    app.add_systems(
        Update,
        (
            game_loop_driver,
            simulate_gameplay_mutations,
            reactive_added_filter_system,
            reactive_changed_filter_system,
            granular_tracker_inspection_system,
        ),
    );

    app.set_runner(test_runner_four_frames);
    app.run();
}
