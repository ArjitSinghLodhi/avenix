#![allow(dead_code)]
#![allow(unused_variables)]

use avenix::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

static POS_ADD_TRIGGERS: AtomicU32 = AtomicU32::new(0);
static VEL_OVERWRITE_TRIGGERS: AtomicU32 = AtomicU32::new(0);

fn on_add_pos(_world: &mut DeferredWorld, _entity: &Entity) {
    POS_ADD_TRIGGERS.fetch_add(1, Ordering::SeqCst);
}

fn on_discard_vel(_world: &mut DeferredWorld, _entity: &Entity) {
    VEL_OVERWRITE_TRIGGERS.fetch_add(1, Ordering::SeqCst);
}

#[derive(Component)]
#[component(on_add = on_add_pos)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Component)]
#[component(on_discard = on_discard_vel)]
struct Velocity {
    x: f32,
    y: f32,
}

#[derive(Component)]
struct TagA;

#[derive(Component)]
struct TagB;

#[derive(Resource)]
struct ScoreTracker {
    points: i32,
}

fn test_runner_once(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
}

#[derive(ComponentBundle)]
struct PlayerComponentBundle {
    pos: Position,
    vel: Velocity,
    tag: TagA,
    #[component_bundle(ignore)]
    _marker: std::marker::PhantomData<()>,
}

#[derive(QueryData)]
struct PhysicsQuery {
    pos: &'static Position,
    vel: &'static mut Velocity,
}

#[derive(QueryFilter)]
struct TagAFilter {
    has_a: With<TagA>,
    no_b: Without<TagB>,
}

#[derive(SystemParam)]
struct CompositeSystemParam<'q> {
    query: Query<'q, PhysicsQuery, TagAFilter>,
    tracker: ResMut<'q, ScoreTracker>,
    #[system_param(ignore)]
    local_state: u32,
}

fn setup_macro_entities(commands: Commands) {
    commands.spawn(PlayerComponentBundle {
        pos: Position { x: 50.0, y: 50.0 },
        vel: Velocity { x: 2.0, y: 2.0 },
        tag: TagA,
        _marker: std::marker::PhantomData,
    });

    commands.spawn((
        Position { x: 999.0, y: 999.0 },
        Velocity { x: 0.0, y: 0.0 },
        TagA,
        TagB,
    ));
}

fn verify_and_mutate_macros(mut tools: CompositeSystemParam) {
    tools.tracker.points += 500;
    assert_eq!(tools.local_state, 0);
    let matched_entities = tools.query.filtered_entities();
    for mut entity in tools.query.iter_mut() {
        entity.vel.x += 10.0;
        entity.vel.y += 10.0;
        assert_eq!(entity.pos.x, 50.0);
        assert_eq!(entity.pos.y, 50.0);
        assert_eq!(entity.vel.x, 12.0);
        assert_eq!(entity.vel.y, 12.0);
    }
    assert_eq!(matched_entities, 1);
}

#[test_fork::test]
fn test_macro_derives_lifecycle() {
    POS_ADD_TRIGGERS.store(0, Ordering::SeqCst);
    VEL_OVERWRITE_TRIGGERS.store(0, Ordering::SeqCst);

    let mut app = App::new();
    app.insert_resource(ScoreTracker { points: 0 })
        .add_systems(Startup, setup_macro_entities)
        .add_systems(Update, verify_and_mutate_macros);

    app.set_runner(test_runner_once);
    app.run();

    assert_eq!(POS_ADD_TRIGGERS.load(Ordering::SeqCst), 2);
    assert_eq!(VEL_OVERWRITE_TRIGGERS.load(Ordering::SeqCst), 0);
}
