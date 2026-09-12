use avenix::prelude::*;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

#[derive(Component)]
pub struct Foo(pub f32);
#[derive(Component)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}
#[derive(Component)]
pub struct Health {
    pub hp: i32,
}
#[derive(Component)]
pub struct Renderable;
#[derive(Component)]
pub struct StaticBody;
#[derive(Component)]
pub struct BenchmarkTarget;
#[derive(Resource)]
pub struct BenchmarkTargets {
    pub entities: Vec<Entity>,
}

criterion_main!(benches);

fn setup_fragmented_world(mut commands: Commands) {
    let spawn_count = 2_000_000;
    println!(
        "Allocation Phase: Spawning {} highly fragmented entities...",
        spawn_count
    );

    let mut rng_seed = 0x853C205A_u64;
    let mut assigned_targets = 0;

    for _ in 0..spawn_count {
        rng_seed ^= rng_seed << 13;
        rng_seed ^= rng_seed >> 7;
        rng_seed ^= rng_seed << 17;
        match rng_seed % 5 {
            0 => {
                if assigned_targets < 100_000 {
                    commands.spawn((
                        Foo(assigned_targets as f32),
                        Velocity { x: 1.0, y: 1.0 },
                        BenchmarkTarget,
                    ));
                    assigned_targets += 1;
                } else {
                    commands.spawn((Foo(0.0), Velocity { x: 1.0, y: 1.0 }));
                }
            }
            1 => {
                commands.spawn((Foo(0.0), StaticBody));
            }
            2 => {
                commands.spawn((Foo(0.0), Velocity { x: -1.0, y: 0.0 }, Health { hp: 100 }));
            }
            3 => {
                commands.spawn((Foo(0.0), Renderable));
            }
            _ => {
                commands.spawn(Health { hp: 10 });
            }
        }
    }
    println!(
        "--> Allocation Complete. Targets placed: {}",
        assigned_targets
    );
}

fn collect_and_scramble_targets(
    query: Query<Entity, With<BenchmarkTarget>>,
    mut targets: ResMut<BenchmarkTargets>,
) {
    if !targets.entities.is_empty() {
        return;
    }
    for view in query.iter() {
        for entity in view.iter() {
            targets.entities.push(entity.clone());
        }
    }

    println!(
        "Target Tracking: Isolated exactly {} original benchmark targets.",
        targets.entities.len()
    );
    println!("Scrambling handles to guarantee cold CPU cache line misses...");

    let mut rng_seed = 0x7FFF_FFFF_u64;
    for i in (1..targets.entities.len()).rev() {
        rng_seed ^= rng_seed << 13;
        rng_seed ^= rng_seed >> 7;
        rng_seed ^= rng_seed << 17;
        let j = (rng_seed % (i as u64 + 1)) as usize;
        targets.entities.swap(i, j);
    }
}

fn run_fragmented_criterion_bench(query: Query<&Foo>, targets: Res<BenchmarkTargets>) {
    if targets.entities.is_empty() {
        return;
    }

    let mut c = Criterion::default().configure_from_args();
    let mut group = c.benchmark_group("ecs_massive_fragmentation");
    let target_count = targets.entities.len();

    group.bench_with_input("avenix_safe_fragmented_lookup", &target_count, |b, _| {
        b.iter(|| {
            for entity in &targets.entities {
                if let Some(foo) = query.get(entity) {
                    black_box(foo);
                }
            }
        });
    });

    group.bench_with_input(
        "avenix_unchecked_fragmented_lookup",
        &target_count,
        |b, _| {
            b.iter(|| {
                for entity in &targets.entities {
                    unsafe {
                        black_box(query.get_unchecked(entity));
                    }
                }
            });
        },
    );

    group.finish();
}

fn run_bench_timeline(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
}

fn bench_entry_point(_c: &mut Criterion) {
    App::new()
        .add_plugins(DefaultSchedulesPlugin)
        .insert_resource(BenchmarkTargets {
            entities: Vec::new(),
        })
        .add_systems(Startup, setup_fragmented_world)
        .add_systems(Update, collect_and_scramble_targets)
        .add_systems(Update, run_fragmented_criterion_bench)
        .set_runner(run_bench_timeline)
        .run();
}

criterion_group!(benches, bench_entry_point);
