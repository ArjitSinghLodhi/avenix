use avenix::prelude::*;

#[derive(Component)]
#[component(on_insert = ConnectionSocket::handle_insert, on_remove = ConnectionSocket::handle_remove)]
struct ConnectionSocket {
    remote_node: Entity,
}

impl ConnectionSocket {
    fn handle_insert(world: &mut ExclusiveWorld, entity: &Entity) {
        let mut target_cluster = None;
        if let Some(socket) = world.get::<ConnectionSocket>(entity) {
            target_cluster = Some(socket.remote_node.clone());
        }

        if let Some(cluster_entity) = target_cluster {
            if let Some(mut cluster) = world.get_mut::<SocketCluster>(&cluster_entity) {
                cluster.connected_sockets.push(entity.clone());
            }
        }
    }

    fn handle_remove(world: &mut ExclusiveWorld, entity: &Entity) {
        let mut target_cluster = None;
        if let Some(socket) = world.get::<ConnectionSocket>(entity) {
            target_cluster = Some(socket.remote_node.clone());
        }

        if let Some(cluster_entity) = target_cluster {
            let mut clean_cluster_component = false;
            if let Some(mut cluster) = world.get_mut::<SocketCluster>(&cluster_entity) {
                cluster.connected_sockets.retain(|e| e != entity);
                if cluster.connected_sockets.is_empty() {
                    clean_cluster_component = true;
                }
            }

            if clean_cluster_component {
                world.remove_components::<SocketCluster>(cluster_entity);
            }
        }
    }
}

#[derive(Component)]
#[component(on_remove = SocketCluster::handle_remove)]
struct SocketCluster {
    connected_sockets: Vec<Entity>,
}

impl SocketCluster {
    fn handle_remove(world: &mut ExclusiveWorld, entity: &Entity) {
        let mut sockets_to_clear = Vec::new();
        if let Some(cluster) = world.get::<SocketCluster>(entity) {
            sockets_to_clear = cluster.connected_sockets.clone();
        }

        while let Some(socket_entity) = sockets_to_clear.pop() {
            if world.get::<ConnectionSocket>(&socket_entity).is_some() {
                world.remove_components::<ConnectionSocket>(socket_entity.clone());
                world.despawn(socket_entity);
            }
        }

        if let Some(mut metrics) = world.get_resource_mut::<NetworkTrafficMetrics>() {
            metrics.total_invalidated_clusters += 1;
        }
    }
}

#[derive(Resource)]
struct NetworkTrafficMetrics {
    total_invalidated_clusters: u32,
}

#[derive(Component)]
struct NetworkHostNode;

#[derive(Resource)]
struct SimulationStepper {
    target_frame: u32,
}

fn spawn_network_nodes(commands: Commands) {
    println!("Init Step 1: Batch spawning 10,000 network host nodes with base configurations...");

    for _ in 0..10_000 {
        commands.spawn((
            NetworkHostNode,
            SocketCluster {
                connected_sockets: Vec::with_capacity(20),
            },
        ));
    }
}

fn allocate_dynamic_sockets(commands: Commands, host_query: Query<Entity, With<NetworkHostNode>>) {
    println!("Init Step 2: Batch spawning 200,000 dynamic self-registering sockets...");
    let host_entities: Vec<Entity> = host_query.iter().cloned().collect();

    let socket_iterator = host_entities.into_iter().flat_map(|host_entity| {
        (0..20).map(move |_| ConnectionSocket {
            remote_node: host_entity.clone(),
        })
    });
    commands.spawn_batch(socket_iterator);
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

    println!("\n--- Phase 1: Executing Heavy Load Pass ---");
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
        .insert_resource(NetworkTrafficMetrics {
            total_invalidated_clusters: 0,
        })
        .insert_resource(SimulationStepper { target_frame: 0 })
        .add_systems(
            Startup,
            (spawn_network_nodes, allocate_dynamic_sockets).chain(),
        )
        .add_systems(
            Update,
            (
                update_stepper_frame,
                simulate_network_failure.run_if(
                    |stepper: ParallelResourceAccessor<SimulationStepper>| {
                        stepper.scope(|s| s.target_frame == 1)
                    },
                ),
            ),
        )
        .set_runner(verify_network_convergence)
        .run();
}
