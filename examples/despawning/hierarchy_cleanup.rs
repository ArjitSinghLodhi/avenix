use avenix::prelude::*;

#[derive(Component)]
struct Parent {
    children: Vec<Entity>,
}

#[derive(Component)]
struct Child {
    parent: Entity,
}

#[derive(Component)]
struct LinkTo {
    parent: Entity,
}

#[derive(Component)]
struct AlphaCommander;
#[derive(Component)]
struct BetaCommander;
#[derive(Component)]
struct MinionSubUnit;

pub struct HierarchyPlugin;

impl Plugin for HierarchyPlugin {
    fn build(self, app: &mut App) {
        app.add_systems(Update, automatic_hierarchy_linker_system)
            .add_systems(CleanupHandles, hirearchy_cleanup);
    }
}

fn automatic_hierarchy_linker_system(
    mut commands: Commands,
    mut parent_query: Query<&mut Parent>,
    link_query: Query<(Entity, &LinkTo)>,
) {
    for view in link_query.iter() {
        for (child_entity, link_comp) in view.iter() {
            let parent_target = link_comp.parent.clone();
            #[allow(unused_mut)]
            if let Some(mut parent_comp) = parent_query.get_mut(&parent_target) {
                parent_comp.children.push(child_entity.clone());

                commands.insert_components(
                    child_entity.clone(),
                    Child {
                        parent: parent_target,
                    },
                );
            }

            commands.remove_components::<LinkTo>(child_entity.clone());
        }
    }
}

fn hirearchy_cleanup(
    commands1: Commands,
    mut commands2: Commands,
    mut parent_query: Query<(Entity, &mut Parent)>,
    child_query: Query<(Entity, &Child)>,
    link_query: Query<(Entity, &LinkTo)>,
) {
    for despawn_cmd in commands1.despawn_iter() {
        let dead_entity = despawn_cmd.despawn_target();
        #[allow(unused_mut)]
        if let Some((entity, mut parent_comp)) = parent_query.get_mut(dead_entity) {
            println!("Hierarchy Plugin: Parent matched. Queueing recursive child deletion!");
            while let Some(child_handle) = parent_comp.children.pop() {
                commands2.despawn(child_handle.clone());
                commands2.remove_components::<Child>(child_handle.clone());
            }
            commands2.remove_components::<Parent>(entity.clone());
        }
    }

    for view in child_query.iter() {
        for (child_entity, child_comp) in view.iter() {
            if commands1.will_despawn(&child_comp.parent) {
                println!(
                    "Hierarchy Plugin: Parent is dying. Stripping Child component from entity silently..."
                );
                commands2.remove_components::<Child>(child_entity.clone());
            }
        }
    }
    for view in link_query.iter() {
        for (child_entity, link_comp) in view.iter() {
            if commands1.will_despawn(&link_comp.parent) {
                println!(
                    "Hierarchy Plugin: Target parent is dying before linkage completes. Stripping LinkTo component safely..."
                );
                commands2.remove_components::<LinkTo>(child_entity.clone());
            }
        }
    }
}

#[derive(Resource)]
pub struct FrameStepper {
    pub current_frame: u32,
}

fn main() {
    println!("--- Starting Verified Hierarchy Shifting Test ---");

    App::new()
        .add_plugins(DefaultSchedulesPlugin)
        .add_plugins(HierarchyPlugin)
        .insert_resource(FrameStepper { current_frame: 0 })
        .add_systems(Startup, setup_scene_graph)
        .add_systems(Update, trigger_runtime_lifecycle_stages)
        .set_runner(run_test_frames)
        .run();
}

fn run_test_frames(app: &mut App) {
    app.build();
    app.run_startup();

    println!("\n--- Frame 1: Dynamic Component Linkage ---");
    app.update();

    println!("\n--- Frame 2: Simultaneous LinkTo and Parent Despawn Pass ---");
    app.update();

    println!("\n--- Frame 3: Verification Check Phase ---");
    app.update();

    println!("\nSimulation processed entire lifecycle safely without a handle violation panic!");
}

fn setup_scene_graph(mut commands: Commands) {
    println!("Setup System: Queueing structural entities into the database...");

    commands.spawn((
        AlphaCommander,
        Parent {
            children: Vec::new(),
        },
    ));
    commands.spawn((
        BetaCommander,
        Parent {
            children: Vec::new(),
        },
    ));
    commands.spawn(MinionSubUnit);
}

fn trigger_runtime_lifecycle_stages(
    mut commands: Commands,
    mut stepper: ResMut<FrameStepper>,
    alpha_query: Query<Entity, With<AlphaCommander>>,
    child_query: Query<Entity, (Without<Parent>, Without<Child>, Without<LinkTo>)>,
) {
    stepper.current_frame += 1;
    let frame = stepper.current_frame;

    if frame == 1 {
        for view in alpha_query.iter() {
            for parent_id in view.iter() {
                for child_view in child_query.iter() {
                    for child_entity in child_view.iter() {
                        println!("System (Update): Binding LinkTo component onto child target...");
                        commands.insert_components(
                            child_entity.clone(),
                            LinkTo {
                                parent: parent_id.clone(),
                            },
                        );
                    }
                }
            }
        }
    }

    if frame == 2 {
        for view in alpha_query.iter() {
            for parent_id in view.iter() {
                println!(
                    "System (Update): Spawning a new child entity requesting LinkTo Alpha Commander..."
                );
                commands.spawn((
                    MinionSubUnit,
                    LinkTo {
                        parent: parent_id.clone(),
                    },
                ));

                println!(
                    "System (Update): Simultaneously issuing deferred despawn for Alpha Commander!"
                );
                commands.despawn(parent_id.clone());
            }
        }
    }
}
