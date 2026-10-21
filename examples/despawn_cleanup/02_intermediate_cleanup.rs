use avenix::prelude::*;

#[derive(Component)]
pub struct ConnectionSocket {
    pub remote_node: Entity,
}

#[derive(Component)]
pub struct SocketCluster {
    pub connected_sockets: Vec<Entity>,
}

#[derive(Resource)]
pub struct NetworkTrafficMetrics {
    pub total_invalidated_clusters: u32,
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NetworkTrafficMetrics {
            total_invalidated_clusters: 0,
        })
        .add_systems(
            CleanupHandles,
            (cleanup_dead_sockets, bidirectional_cluster_purge).chain(),
        );
    }
}

fn cleanup_dead_sockets(commands: Commands, socket_query: Query<(Entity, &ConnectionSocket)>) {
    socket_query.par_iter().for_each(|(socket_entity, socket)| {
        if commands.will_despawn(&socket.remote_node) {
            commands
                .entity(&socket_entity)
                .remove::<ConnectionSocket>()
                .despawn();
        }
    });
}

fn bidirectional_cluster_purge(
    commands: Commands,
    mut cluster_query: Query<&mut SocketCluster>,
    mut metrics: ResMut<NetworkTrafficMetrics>,
) {
    commands.for_each_despawn(|despawned_node| {
        #[allow(unused_mut)]
        if let Some(mut cluster) = cluster_query.get_mut(despawned_node) {
            for socket_entity in cluster.connected_sockets.drain(..) {
                if !commands.will_despawn(&socket_entity) {
                    commands
                        .entity(&socket_entity)
                        .remove::<ConnectionSocket>()
                        .despawn();
                }
            }
            metrics.total_invalidated_clusters += 1;
        }
    });
}

#[derive(Component)]
struct NetworkHostNode;

#[derive(Resource)]
struct SimulationStepper {
    target_frame: u32,
}

fn spawn_network_nodes(commands: Commands) {
    println!("Init: Batch spawning 10,000 network host infrastructure nodes...");
    commands.spawn_batch((0..10_000).map(|_| NetworkHostNode));
}

fn link_bidirectional_sockets(
    commands: Commands,
    host_query: Query<Entity, With<NetworkHostNode>>,
    socket_query: Query<Entity, With<ConnectionSocket>>,
) {
    println!("Init: Batch spawning 200,000 dynamic sockets mapped back to host architectures...");

    let host_entities: Vec<Entity> = host_query.iter().cloned().collect();

    let socket_iterator = host_entities.into_iter().flat_map(|host_entity| {
        (0..20).map(move |_| ConnectionSocket {
            remote_node: host_entity.clone(),
        })
    });
    commands.spawn_batch(socket_iterator);

    let mut all_sockets = Vec::new();
    for socket_entity in socket_query.iter() {
        all_sockets.push(socket_entity.clone());
    }

    let mut socket_chunks = all_sockets.chunks_exact(20);
    for host_entity in host_query.iter() {
        if let Some(chunk) = socket_chunks.next() {
            commands.entity(&host_entity).insert(SocketCluster {
                connected_sockets: chunk.to_vec(),
            });
        }
    }
}

fn simulate_network_failure(commands: Commands, host_query: Query<Entity, With<NetworkHostNode>>) {
    println!("\n[Simulation] Cascade Drop Event: Purging 5,000 host nodes simultaneously...");
    for (i, host_entity) in host_query.iter().enumerate() {
        if i % 2 == 0 {
            commands.entity(&host_entity).despawn();
        }
    }
}

fn verify_network_convergence(app: &mut App) {
    app.build();
    app.run_startup();

    println!("\n--- Phase 1: Executing Heavy Load Parallel Cleanup Pass ---");
    app.update();

    let metrics = app.world().resource::<NetworkTrafficMetrics>();
    println!(
        "\n[Results] Traversal Complete. Invalidated Clusters Count: {}",
        metrics.total_invalidated_clusters
    );
    println!(
        "Simulation converged with large entity volume without triggering handle leak panics!"
    );
}

fn update_stepper_frame(mut stepper: ResMut<SimulationStepper>) {
    stepper.target_frame += 1;
}

fn main() {
    println!("--- Initializing Heavy Load Direct Lookup Pipeline ---");

    App::new()
        .add_plugins(NetworkPlugin)
        .insert_resource(SimulationStepper { target_frame: 0 })
        .add_systems(
            Startup,
            (spawn_network_nodes, link_bidirectional_sockets).chain(),
        )
        .add_systems(
            Update,
            (
                update_stepper_frame,
                simulate_network_failure.run_if(
                    |stepper: &ParallelResourceAccessor<SimulationStepper>| {
                        stepper.scope(|s| s.target_frame == 1)
                    },
                ),
            ),
        )
        .set_runner(verify_network_convergence)
        .run();
}
