use avenix::prelude::*;

#[derive(Debug, Component)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Debug, Component)]
struct Velocity {
    x: f32,
    y: f32,
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Enemy;

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
    tag: Player,
}

#[derive(QueryData)]
#[avenix(query_data(derive(Debug)))]
struct PhysicsQuery {
    pos: &'static Position,
    vel: &'static mut Velocity,
}

#[derive(QueryFilter)]
struct PlayerFilter {
    has_player: With<Player>,
    not_enemy: Without<Enemy>,
}

#[derive(SystemParam)]
struct CompositeSystemParam<'a> {
    query: Query<'a, PhysicsQuery, PlayerFilter>,
    tracker: ResMut<'a, ScoreTracker>,
    #[avenix(system_param(ignore))]
    // Initialized with Default::default() every frame, it does not persist accross frames
    counter: usize,
}

fn setup_macro_entities(mut commands: Commands) {
    commands.spawn(PlayerComponentBundle {
        pos: Position { x: 50.0, y: 50.0 },
        vel: Velocity { x: 2.0, y: 2.0 },
        tag: Player,
    });

    commands.spawn((
        Position { x: 999.0, y: 999.0 },
        Velocity { x: 0.0, y: 0.0 },
        Enemy,
    ));
}

fn verify_and_mutate_macros(mut tools: CompositeSystemParam) {
    let mut matched_entities = 0;
    tools.tracker.points += 500;

    for mut view in tools.query.iter_mut() {
        matched_entities += view.len();
        #[allow(unused_mut)]
        for mut entity in view.iter_mut() {
            println!("entity: {:?}", entity);
            entity.vel.x += 10.0;
            entity.vel.y += 10.0;
            assert_eq!(entity.pos.x, 50.0);
            assert_eq!(entity.pos.y, 50.0);
            assert_eq!(entity.vel.x, 12.0);
            assert_eq!(entity.vel.y, 12.0);
        }
    }
    tools.counter += 1;
    assert_eq!(matched_entities, 1);
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultSchedulesPlugin)
        .insert_resource(ScoreTracker { points: 0 })
        .add_systems(Startup, setup_macro_entities)
        .add_systems(Update, verify_and_mutate_macros);

    app.set_runner(test_runner_once);
    app.run();
    let tracker = app.world().get_resource::<ScoreTracker>();
    assert_eq!(tracker.points, 500);
}
