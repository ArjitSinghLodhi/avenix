use avenix::prelude::*;

#[derive(Component)]
struct LeakedTarget;

#[allow(dead_code)]
#[derive(Resource)]
struct StaleHandleContainer {
    pub leaked_handle: Option<Entity>,
}

fn spawn_leaked_target(commands: Commands) {
    commands.spawn(LeakedTarget);
}

fn capture_and_leak_handle(commands: Commands, query: Query<Entity, With<LeakedTarget>>) {
    if let Some(target) = query.iter().next() {
        commands.insert_resource(StaleHandleContainer {
            leaked_handle: Some(target.clone()),
        });
    }
}

fn trigger_uncoordinated_despawn(commands: Commands, query: Query<Entity, With<LeakedTarget>>) {
    if let Some(target_entity) = query.iter().next() {
        commands.entity(&target_entity).despawn();
    }
}

#[test_fork::test]
#[should_panic(expected = "AvenixEntityHandleViolation")]
fn test_forgotten_handle_scrub_triggers_panic() {
    let mut app = App::new();

    app.add_systems(Startup, spawn_leaked_target).add_systems(
        Update,
        (capture_and_leak_handle, trigger_uncoordinated_despawn).chain(),
    );

    app.build();
    app.run_startup();
    app.update();
}
