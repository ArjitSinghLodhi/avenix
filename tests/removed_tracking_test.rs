use avenix::prelude::*;
use rusty_fork::rusty_fork_test;

#[derive(PartialEq, Component)]
enum EntityTag {
    RemovalTarget,
    DespawnTarget,
    UntouchedNeighbor,
}

#[derive(Component)]
#[allow(dead_code)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Component)]
#[allow(dead_code)]
struct Velocity {
    x: f32,
    y: f32,
}

#[derive(Resource)]
struct FrameCounter {
    current_frame: u32,
}

fn increment_frame_system(mut counter: ResMut<FrameCounter>) {
    counter.current_frame += 1;
}

fn setup_removal_entities(mut commands: Commands) {
    commands.spawn((
        Position { x: 1.0, y: 1.0 },
        Velocity { x: 5.0, y: 5.0 },
        EntityTag::RemovalTarget,
    ));
    commands.spawn((
        Position { x: 1.5, y: 1.5 },
        Velocity { x: 2.0, y: 2.0 },
        EntityTag::DespawnTarget,
    ));
    commands.spawn((Position { x: 2.0, y: 2.0 }, EntityTag::UntouchedNeighbor));
}

fn apply_frame_1_mutations(
    counter: Res<FrameCounter>,
    query: Query<(Entity, &EntityTag)>,
    mut commands: Commands,
) {
    if counter.current_frame != 1 {
        return;
    }

    for view in query.iter() {
        for (entity, tag) in view.iter() {
            match tag {
                EntityTag::RemovalTarget => {
                    commands.remove_components::<Velocity>(entity.clone());
                }
                EntityTag::DespawnTarget => {
                    commands.remove_components::<Velocity>(entity.clone());
                    commands.despawn(entity.clone());
                }
                EntityTag::UntouchedNeighbor => {}
            }
        }
    }
}

fn verify_frame_1_removal_isolation(
    counter: Res<FrameCounter>,
    removed_vel: RemovedComponents<Velocity>,
) {
    if counter.current_frame != 1 {
        return;
    }

    let mut removal_visible_early = false;
    for _entity in removed_vel.iter() {
        removal_visible_early = true;
    }
    assert!(
        !removal_visible_early,
        "Double buffer leak! RemovedComponents<Velocity> visible on write frame."
    );
}

fn verify_frame_2_removal_reads(
    counter: Res<FrameCounter>,
    removed_vel: RemovedComponents<Velocity>,
    query_neighbor: Query<&EntityTag, (With<Position>, Without<Velocity>)>,
) {
    if counter.current_frame != 2 {
        return;
    }

    let mut tracking_count = 0;
    for _entity in removed_vel.iter() {
        tracking_count += 1;
    }

    assert_eq!(
        tracking_count, 1,
        "Engine failure! Expected exactly 1 entity removal handle, found {}.",
        tracking_count
    );

    let mut neighbor_intact = false;
    for view in query_neighbor.iter() {
        for tag in view.iter() {
            if *tag == EntityTag::UntouchedNeighbor {
                neighbor_intact = true;
            }
        }
    }
    assert!(
        neighbor_intact,
        "Archetype neighbor layout was corrupted during buffered component removals."
    );
}

fn verify_frame_3_removal_decay(
    counter: Res<FrameCounter>,
    removed_vel: RemovedComponents<Velocity>,
) {
    if counter.current_frame != 3 {
        return;
    }

    let mut removal_active = false;
    for _entity in removed_vel.iter() {
        removal_active = true;
    }
    assert!(
        !removal_active,
        "RemovedComponents<Velocity> failed to naturally decay after its visibility frame."
    );
}

rusty_fork_test! {
    #[test]
    fn test_double_buffered_removals() {
        let mut app = App::new();
        app.add_plugins(DefaultSchedulesPlugin)
            .insert_resource(FrameCounter { current_frame: 0 })
            .add_systems(Startup, setup_removal_entities)
            .add_systems(
                Update,
                (
                    increment_frame_system,
                    apply_frame_1_mutations,
                    verify_frame_1_removal_isolation,
                    verify_frame_2_removal_reads,
                    verify_frame_3_removal_decay,
                ),
            );

        app.set_runner(test_runner_removals);
        app.run();
    }
}

fn test_runner_removals(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
    app.update();
    app.update();
}
