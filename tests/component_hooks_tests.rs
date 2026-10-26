use avenix::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

static SPAWNED_CHILDREN: AtomicU32 = AtomicU32::new(0);
static TARGET_CLEANUP_RUN: AtomicU32 = AtomicU32::new(0);
static INSERT_EXEC_COUNT: AtomicU32 = AtomicU32::new(0);
static MANUAL_ADD_COUNT: AtomicU32 = AtomicU32::new(0);

#[allow(dead_code)]
#[derive(Component)]
struct Transform {
    x: f32,
    y: f32,
}

fn on_discard_physics(world: &mut ExclusiveWorld, entity: &Entity) {
    let mut meets_condition = false;

    {
        if let Some(old_body) = world.get::<PhysicsBody>(entity) {
            if old_body.mass > 50.0 {
                meets_condition = true;
            }
        }
    }

    if meets_condition {
        SPAWNED_CHILDREN.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Component)]
#[component(on_discard = on_discard_physics)]
struct PhysicsBody {
    mass: f32,
}

fn on_add_physics(world: &mut ExclusiveWorld, entity: &Entity) {
    let mut meets_condition = false;

    {
        if let Some(transform) = world.get::<Transform>(entity) {
            if transform.x > 10.0 {
                meets_condition = true;
            }
        }
    }

    if meets_condition {
        world.insert_components(entity.clone(), (MarkerComponent, JointComponent));
    }
}

fn on_insert_physics(_world: &mut ExclusiveWorld, _entity: &Entity) {
    INSERT_EXEC_COUNT.fetch_add(1, Ordering::SeqCst);
}

fn on_remove_physics(_world: &mut ExclusiveWorld, _entity: &Entity) {
    TARGET_CLEANUP_RUN.fetch_add(1, Ordering::SeqCst);
}

#[allow(dead_code)]
#[derive(Component)]
#[component(on_add = on_add_physics, on_insert = on_insert_physics, on_remove = on_remove_physics)]
struct PhysicsPluginComponent {
    mass: f32,
}

#[derive(Component)]
struct MarkerComponent;

#[derive(Component)]
struct JointComponent;

#[derive(Component)]
struct ManuallyRegisteredComponent;

fn reset_trackers() {
    SPAWNED_CHILDREN.store(0, Ordering::SeqCst);
    TARGET_CLEANUP_RUN.store(0, Ordering::SeqCst);
    INSERT_EXEC_COUNT.store(0, Ordering::SeqCst);
    MANUAL_ADD_COUNT.store(0, Ordering::SeqCst);
}

#[test_fork::test]
fn test_hook_reads_sibling_component_and_mutates_structure() {
    reset_trackers();
    let mut app = App::new();
    app.build();
    app.run_startup();

    let par_commands = app.world_mut().par_commands();
    par_commands.scope(|cmd| {
        cmd.spawn((
            Transform { x: 15.0, y: 0.0 },
            PhysicsPluginComponent { mass: 10.0 },
        ));
    });
    app.update();

    assert_eq!(INSERT_EXEC_COUNT.load(Ordering::SeqCst), 1);

    let query_accessor = app.world().par_query_accessor::<(Option<&MarkerComponent>, Option<&JointComponent>), EmptyQueryFilter>();
    query_accessor.scope(|query| {
        let mut found = false;
        for (marker, joint) in query.iter() {
            assert!(marker.is_some());
            assert!(joint.is_some());
            found = true;
        }
        assert!(found);
    });
}

#[test_fork::test]
fn test_hook_reads_previous_data_and_spawns_via_world_commands() {
    reset_trackers();
    let mut app = App::new();
    app.build();
    app.run_startup();

    let par_commands = app.world_mut().par_commands();
    par_commands.scope(|cmd| {
        cmd.spawn(PhysicsBody { mass: 60.0 });
    });
    app.update();

    let query_accessor = app.world().par_query_accessor::<Entity, EmptyQueryFilter>();
    let mut targets = Vec::new();
    query_accessor.scope(|query| {
        for entity in query.iter() {
            targets.push(entity.clone());
        }
    });

    for entity in targets {
        let par_commands = app.world_mut().par_commands();
        par_commands.scope(|cmd| {
            cmd.entity(&entity).insert(PhysicsBody { mass: 20.0 });
        });
    }
    app.update();

    assert_eq!(SPAWNED_CHILDREN.load(Ordering::SeqCst), 1);
}

#[test_fork::test]
fn test_hook_intercepts_subtraction_and_cleans_dependent_components() {
    reset_trackers();
    let mut app = App::new();
    app.build();
    app.run_startup();

    let par_commands = app.world_mut().par_commands();
    par_commands.scope(|cmd| {
        cmd.spawn((
            Transform { x: 0.0, y: 0.0 },
            PhysicsPluginComponent { mass: 10.0 },
        ));
    });
    app.update();

    assert_eq!(INSERT_EXEC_COUNT.load(Ordering::SeqCst), 1);

    let query_accessor = app.world().par_query_accessor::<Entity, EmptyQueryFilter>();
    let mut targets = Vec::new();
    query_accessor.scope(|query| {
        for entity in query.iter() {
            targets.push(entity.clone());
        }
    });

    for entity in targets {
        let par_commands = app.world_mut().par_commands();
        par_commands.scope(|cmd| {
            cmd.entity(&entity).remove::<PhysicsPluginComponent>();
        });
    }
    app.update();

    assert_eq!(TARGET_CLEANUP_RUN.load(Ordering::SeqCst), 1);
}

#[test_fork::test]
fn test_manual_hooks_registration_works_on_spawn() {
    reset_trackers();
    let mut app = App::new();

    app.world_mut()
        .register_component_hooks::<ManuallyRegisteredComponent>()
        .on_add(|_world, _entity| {
            MANUAL_ADD_COUNT.fetch_add(1, Ordering::SeqCst);
        });

    app.build();
    app.run_startup();

    let par_commands = app.world_mut().par_commands();
    par_commands.scope(|cmd| {
        cmd.spawn(ManuallyRegisteredComponent);
    });
    app.update();

    assert_eq!(MANUAL_ADD_COUNT.load(Ordering::SeqCst), 1);
}

#[test_fork::test]
#[should_panic(expected = "AvenixComponentHooksError")]
fn test_manual_hooks_registration_panics_on_duplicate() {
    let mut app = App::new();

    app.world_mut()
        .register_component_hooks::<ManuallyRegisteredComponent>()
        .on_add(|_world, _entity| {});

    app.world_mut()
        .register_component_hooks::<ManuallyRegisteredComponent>()
        .on_add(|_world, _entity| {});
}

#[test_fork::test]
#[should_panic(expected = "AvenixComponentHooksError")]
fn test_manual_hooks_registration_panics_after_entity_spawned() {
    let mut app = App::new();
    app.build();
    app.run_startup();

    let par_commands = app.world_mut().par_commands();
    par_commands.scope(|cmd| {
        cmd.spawn(ManuallyRegisteredComponent);
    });
    app.update();

    app.world_mut()
        .register_component_hooks::<ManuallyRegisteredComponent>()
        .on_add(|_world, _entity| {});
}
