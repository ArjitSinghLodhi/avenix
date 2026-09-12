use avenix::prelude::*;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

#[derive(Component)]
pub struct Transform {
    pub matrix: [f32; 16],
}
#[derive(Component)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
#[derive(Component)]
pub struct Rotation {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
#[derive(Component)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Resource)]
pub struct BenchState {
    pub initialized: bool,
}

criterion_main!(benches);

fn setup_simple_iter_world(mut commands: Commands, mut state: ResMut<BenchState>) {
    println!("[ECS Bench Setup] Spawning 10,000 dense entities...");

    for _ in 0..10_000 {
        commands.spawn((
            Transform { matrix: [0.0; 16] },
            Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Rotation {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Velocity {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        ));
    }

    state.initialized = true;
    println!("[ECS Bench Setup] Population complete.");
}

fn run_simple_iter_bench(mut query: Query<(&mut Position, &Velocity)>, state: Res<BenchState>) {
    if !state.initialized {
        return;
    }

    let mut c = Criterion::default().configure_from_args();

    c.bench_function("ecs_suite_simple_iter", |b| {
        b.iter(|| {
            for mut view in query.iter_mut() {
                #[allow(unused_mut)]
                for (mut pos, vel) in view.iter_mut() {
                    pos.x += vel.x;
                    pos.y += vel.y;
                    pos.z += vel.z;
                    black_box(pos);
                }
            }
        });
    });
}

fn run_bench_timeline(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
}

fn bench_entry_point(_c: &mut Criterion) {
    App::new()
        .add_plugins(DefaultSchedulesPlugin)
        .insert_resource(BenchState { initialized: false })
        .add_systems(Startup, setup_simple_iter_world)
        .add_systems(Update, run_simple_iter_bench)
        .set_runner(run_bench_timeline)
        .run();
}

criterion_group!(benches, bench_entry_point);
