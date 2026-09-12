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

#[derive(Default, Resource)]
pub struct BenchState {
    pub initialized: bool,
}

criterion_main!(benches);

fn dummy_system_to_force_tracking_allocation(_query: Query<&Position, Changed<Position>>) {}

fn bench_tracked_but_unfiltered(
    mut query_standard: Query<(&mut Position, &Velocity)>,
    state: Res<BenchState>,
) {
    if !state.initialized {
        return;
    }

    let mut c = Criterion::default().configure_from_args();

    c.bench_function("tracked_but_unfiltered_write", |b| {
        b.iter(|| {
            for mut view in query_standard.iter_mut() {
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

fn bench_tracked_and_filtered(
    mut query_reactive: Query<(&mut Position, &Velocity), Changed<Position>>,
    state: Res<BenchState>,
) {
    if !state.initialized {
        return;
    }

    let mut c = Criterion::default().configure_from_args();

    c.bench_function("tracked_and_filtered_write", |b| {
        b.iter(|| {
            for mut view in query_reactive.iter_mut() {
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

fn setup_world(mut commands: Commands, mut state: ResMut<BenchState>) {
    println!(
        "[Tracked Bench] Spawning 10,000 entities with explicit Component Tracking enabled..."
    );
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
}

fn run_bench_timeline(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
}

fn bench_entry_point(_c: &mut Criterion) {
    App::new()
        .add_plugins(DefaultSchedulesPlugin)
        .add_systems(Update, dummy_system_to_force_tracking_allocation)
        .insert_resource(BenchState::default())
        .add_systems(Startup, setup_world)
        .add_systems(Update, bench_tracked_but_unfiltered)
        .add_systems(Update, bench_tracked_and_filtered)
        .set_runner(run_bench_timeline)
        .run();
}

criterion_group!(benches, bench_entry_point);
