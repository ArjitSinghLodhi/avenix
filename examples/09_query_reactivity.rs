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
    println!("\n--- [FRAME {}] ---", counter.frame);
}

fn setup_game(commands: Commands) {
    println!("[Setup/Frame 0] Spawning player entity with a baseline Score.");
    commands.spawn((Player, Score(0)));
}

fn simulate_gameplay_mutations_frame_1(commands: Commands, score_query: Query<(Entity, &Score)>) {
    for (entity, _) in score_query.iter() {
        println!("[Mutation - Frame 1] commands.add_components() queued for 'Buff'.");
        commands.entity(entity.clone()).add(Buff { name: "Haste" });
    }
}

fn simulate_gameplay_mutations_frame_2(
    commands: Commands,
    mut score_query: Query<(Entity, &mut Score)>,
) {
    for (entity, mut score) in score_query.iter_mut() {
        println!("[Mutation - Frame 2] Direct Mut Write modifying Score component value.");
        score.0 = 100;

        println!("[Mutation - Frame 2] commands.insert_components() queued for 'StatusEffect'.");
        commands.entity(entity.clone()).insert(StatusEffect);
    }
}

fn reactive_added_filter_frame_1(
    added_buffs: Query<&Buff, Added<Buff>>,
    added_effects: Query<&StatusEffect, Added<StatusEffect>>,
) {
    for buff in added_buffs.iter() {
        println!(
            "[Reactive Filter] Added<Buff> caught structural addition: '{}'",
            buff.name
        );
    }

    for _ in added_effects.iter() {
        println!("[Reactive Filter] Added<StatusEffect> caught secondary insert structural split!");
    }
}

fn reactive_added_filter_frame_2(
    added_buffs: Query<&Buff, Added<Buff>>,
    added_effects: Query<&StatusEffect, Added<StatusEffect>>,
) {
    for buff in added_buffs.iter() {
        println!(
            "[Reactive Filter] Added<Buff> caught structural addition: '{}'",
            buff.name
        );
    }

    for _ in added_effects.iter() {
        println!("[Reactive Filter] Added<StatusEffect> caught secondary insert structural split!");
    }
}

fn reactive_changed_filter_frame_2(changed_scores: Query<&Score, Changed<Score>>) {
    for score in changed_scores.iter() {
        println!(
            "[Reactive Filter] Changed<Score> filter caught historical mutation! Value: {}",
            score.0
        );
    }
}

fn reactive_changed_filter_frame_4(changed_scores: Query<&Score, Changed<Score>>) {
    let count = changed_scores.total_entities();
    println!(
        "[Lifecycle Decay] Frame 4 Changed<Score> item count: {} (State safely decayed to normal).",
        count
    );
}

fn complex_combinator_filter_frame_1(
    reactive_or_query: Query<(Entity, &Score), Or<(Added<Buff>, Changed<Score>)>>,
    stable_entities_query: Query<Entity, (With<Player>, Not<Added<StatusEffect>>)>,
    mixed_or_query: Query<Entity, Or<(Changed<Score>, With<Buff>)>>,
) {
    for (entity, score) in reactive_or_query.iter() {
        println!(
            "[Or Combinator] Frame 1 - Or<(Added<Buff>, Changed<Score>)> matched entity {:?} (Score: {})",
            entity, score.0
        );
    }

    let stable_count = stable_entities_query.total_entities();
    if stable_count > 0 {
        println!(
            "[Not Modifier] Frame 1 - Not<Added<StatusEffect>> matched {} stable entity/entities.",
            stable_count
        );
    }

    for entity in mixed_or_query.iter() {
        println!(
            "[Mixed Or Branch] Frame 1 - Or<(Changed<Score>, With<Buff>)> safely evaluated table layouts & matched: {:?}",
            entity
        );
    }
}

fn complex_combinator_filter_frame_2(
    reactive_or_query: Query<(Entity, &Score), Or<(Added<Buff>, Changed<Score>)>>,
    stable_entities_query: Query<Entity, (With<Player>, Not<Added<StatusEffect>>)>,
    mixed_or_query: Query<Entity, Or<(Changed<Score>, With<Buff>)>>,
) {
    for (entity, score) in reactive_or_query.iter() {
        println!(
            "[Or Combinator] Frame 2 - Or<(Added<Buff>, Changed<Score>)> matched entity {:?} (Score: {})",
            entity, score.0
        );
    }

    let stable_count = stable_entities_query.total_entities();
    if stable_count > 0 {
        println!(
            "[Not Modifier] Frame 2 - Not<Added<StatusEffect>> matched {} stable entity/entities.",
            stable_count
        );
    }

    for entity in mixed_or_query.iter() {
        println!(
            "[Mixed Or Branch] Frame 2 - Or<(Changed<Score>, With<Buff>)> safely evaluated table layouts & matched: {:?}",
            entity
        );
    }
}

fn granular_tracker_inspection_frame_1(tracker_query: Query<(&Score, ChangedTracker<Score>)>) {
    for (score, tracker) in tracker_query.iter() {
        if tracker.is_changed() {
            println!(
                "[Tracker Inspect] Frame 1 - ChangedTracker detected entry delta without narrowing query array. (Score: {})",
                score.0
            );
        }
    }
}

fn granular_tracker_inspection_frame_2(tracker_query: Query<(&Score, ChangedTracker<Score>)>) {
    for (score, tracker) in tracker_query.iter() {
        if tracker.is_changed() {
            println!(
                "[Tracker Inspect] Frame 2 - ChangedTracker detected entry delta without narrowing query array. (Score: {})",
                score.0
            );
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

    app.insert_resource(GameLoopCounter { frame: 0 })
        .add_systems(Startup, setup_game);

    app.add_systems(
        Update,
        (
            game_loop_driver,
            simulate_gameplay_mutations_frame_1.run_if(
                |counter: &ParallelResourceAccessor<GameLoopCounter>| {
                    counter.scope(|c| c.frame == 1)
                },
            ),
            simulate_gameplay_mutations_frame_2.run_if(
                |counter: &ParallelResourceAccessor<GameLoopCounter>| {
                    counter.scope(|c| c.frame == 2)
                },
            ),
            reactive_added_filter_frame_1.run_if(
                |counter: &ParallelResourceAccessor<GameLoopCounter>| {
                    counter.scope(|c| c.frame == 1)
                },
            ),
            reactive_added_filter_frame_2.run_if(
                |counter: &ParallelResourceAccessor<GameLoopCounter>| {
                    counter.scope(|c| c.frame == 2)
                },
            ),
            reactive_changed_filter_frame_2.run_if(
                |counter: &ParallelResourceAccessor<GameLoopCounter>| {
                    counter.scope(|c| c.frame == 2)
                },
            ),
            reactive_changed_filter_frame_4.run_if(
                |counter: &ParallelResourceAccessor<GameLoopCounter>| {
                    counter.scope(|c| c.frame == 4)
                },
            ),
            complex_combinator_filter_frame_1.run_if(
                |counter: &ParallelResourceAccessor<GameLoopCounter>| {
                    counter.scope(|c| c.frame == 1)
                },
            ),
            complex_combinator_filter_frame_2.run_if(
                |counter: &ParallelResourceAccessor<GameLoopCounter>| {
                    counter.scope(|c| c.frame == 2)
                },
            ),
            granular_tracker_inspection_frame_1.run_if(
                |counter: &ParallelResourceAccessor<GameLoopCounter>| {
                    counter.scope(|c| c.frame == 1)
                },
            ),
            granular_tracker_inspection_frame_2.run_if(
                |counter: &ParallelResourceAccessor<GameLoopCounter>| {
                    counter.scope(|c| c.frame == 2)
                },
            ),
        )
            .chain(),
    );

    app.set_runner(test_runner_four_frames);
    app.run();
}
