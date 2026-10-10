use avenix::prelude::*;
use rayon::iter::ParallelIterator;

#[allow(dead_code)]
#[derive(Component)]
struct WorkerId(u32);

#[derive(Component)]
#[allow(dead_code)]
struct Position {
    x: f32,
    y: f32,
}
#[derive(Component)]
struct Velocity {
    x: f32,
    y: f32,
}

fn main() {
    println!("=== Parallel query example ===\n");

    let mut app = App::new();
    app.add_systems(Startup, spawn_parallel_batch_system)
        .add_systems(
            Update,
            (
                parallel_read_system,
                parallel_write_system,
                parallel_read_fold_system,
                parallel_chunk_read_system,
                parallel_chunk_write_system,
            )
                .chain(),
        );

    app.set_runner(run_parallel_test_loop);
    app.run();

    println!("\n=== Parallel query example completed ===");
}

fn spawn_parallel_batch_system(commands: Commands) {
    println!("Spawning dense data pools via fast batch allocation with predictable noise...");

    let batch = (0..20_000).map(|i| {
        let seed_x = (i as u64).wrapping_mul(48271).wrapping_add(12345);
        let seed_y = seed_x.wrapping_mul(48271).wrapping_add(67890);
        let seed_vx = seed_y.wrapping_mul(48271).wrapping_add(11111);
        let seed_vy = seed_vx.wrapping_mul(48271).wrapping_add(22222);
        let pos_x = (seed_x % 1000) as f32 / 10.0;
        let pos_y = (seed_y % 1000) as f32 / 10.0;
        let vel_x = ((seed_vx % 100) as f32 / 20.0) + 1.0;
        let vel_y = ((seed_vy % 100) as f32 / 20.0) + 1.0;

        (
            Position { x: pos_x, y: pos_y },
            Velocity { x: vel_x, y: vel_y },
            WorkerId(0),
        )
    });

    commands.spawn_batch(batch);
}

fn parallel_read_system(query: Query<(&Position, &WorkerId)>) {
    query.par_iter().for_each(|(_pos, _id)| {});
}

fn parallel_write_system(mut query: Query<(&mut Position, &Velocity)>) {
    #[allow(unused_mut)]
    query.par_iter_mut().for_each(|(mut pos, vel)| {
        pos.x += vel.x;
        pos.y += vel.y;
    });
}

fn parallel_read_fold_system(query: Query<(&Position, &Velocity)>) {
    let (avg_x, avg_y, total_count) = query
        .par_iter()
        .fold(
            || (0.0f32, 0.0f32, 0usize),
            |(mut menu_x, mut menu_y, mut count), (_pos, vel)| {
                count += 1;
                let weight = 1.0 / count as f32;
                menu_x += (vel.x - menu_x) * weight;
                menu_y += (vel.y - menu_y) * weight;
                (menu_x, menu_y, count)
            },
        )
        .reduce(
            || (0.0f32, 0.0f32, 0usize),
            |(a_x, a_y, a_count), (b_x, b_y, b_count)| {
                let total = a_count + b_count;
                if total == 0 {
                    return (0.0, 0.0, 0);
                }

                let weight_b = b_count as f32 / total as f32;
                let combined_x = a_x + (b_x - a_x) * weight_b;
                let combined_y = a_y + (b_y - a_y) * weight_b;

                (combined_x, combined_y, total)
            },
        );

    println!(
        "[par_iter fold] Total rows processed: {} | Native Average Velocity: ({:.2}, {:.2})",
        total_count, avg_x, avg_y
    );
}

fn parallel_chunk_read_system(query: Query<(&Position, &Velocity)>) {
    query
        .par_chunks(5000)
        .for_each(|chunk| for (_pos, _vel) in chunk.iter() {});
}

fn parallel_chunk_write_system(mut query: Query<(&mut Position, &Velocity)>) {
    query.par_chunks_mut(5000).for_each(|mut chunk| {
        #[allow(unused_mut)]
        for (mut pos, vel) in chunk.iter_mut() {
            pos.x += vel.x;
            pos.y += vel.y;
        }
    });
}

fn run_parallel_test_loop(app: &mut App) {
    app.build();
    app.run_startup();

    app.update();
    app.update();
}
