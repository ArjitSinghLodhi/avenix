use avenix::prelude::*;

#[derive(Component)]
pub struct Children {
    pub children: Vec<Entity>,
}

#[derive(Component)]
pub struct ChildOf {
    pub parent: Entity,
}

#[derive(Component)]
pub struct LinkTo {
    pub parent: Entity,
}

#[derive(Component)]
pub struct UnlinkChild;

pub struct HierarchyPlugin;

impl Plugin for HierarchyPlugin {
    fn build(self, app: &mut App) {
        app.add_systems(Update, automatic_hierarchy_linker_system)
            .add_systems(CleanupHandles, hirearchy_cleanup);
    }
}

fn automatic_hierarchy_linker_system(
    commands: Commands,
    mut parent_query: Query<&mut Children>,
    link_query: Query<(Entity, &LinkTo)>,
    unlink_query: Query<(Entity, &ChildOf), With<UnlinkChild>>,
) {
    for view in link_query.iter() {
        for (child_entity, link_comp) in view.iter() {
            let parent_target = link_comp.parent.clone();
            #[allow(unused_mut)]
            if let Some(mut parent_comp) = parent_query.get_mut(&parent_target) {
                parent_comp.children.push(child_entity.clone());
            } else {
                commands.entity(parent_target.clone()).add(Children {
                    children: vec![child_entity.clone()],
                });
            }
            commands
                .entity(child_entity.clone())
                .insert(ChildOf {
                    parent: parent_target,
                })
                .remove::<LinkTo>();
        }
    }
    for view in unlink_query.iter() {
        for (child_entity, child_of) in view.iter() {
            #[allow(unused_mut)]
            if let Some(mut parent_comp) = parent_query.get_mut(&child_of.parent) {
                parent_comp.children.retain(|entity| entity != child_entity);
                println!("Hierarchy Plugin: Unlinked child from parent");
                if parent_comp.children.is_empty() {
                    println!(
                        "Hierarchy Plugin: Removing Children component from parent because there are no children anymore"
                    );
                    commands
                        .entity(child_of.parent.clone())
                        .remove::<Children>();
                }
            }
            commands
                .entity(child_entity.clone())
                .remove::<(UnlinkChild, ChildOf)>();
        }
    }
}

fn hirearchy_cleanup(
    commands: Commands,
    mut parent_query: Query<&mut Children>,
    link_query: Query<(Entity, &LinkTo)>,
    unlink_query: Query<(Entity, &ChildOf), With<UnlinkChild>>,
    unlink_orphan_query: Query<Entity, (With<UnlinkChild>, Without<ChildOf>)>,
    child_query: Query<(Entity, &ChildOf)>,
) {
    for despawn_cmd in commands.despawn_iter() {
        let dead_entity = despawn_cmd.despawn_target();
        #[allow(unused_mut)]
        if let Some(mut parent_comp) = parent_query.get_mut(dead_entity) {
            println!("Hierarchy Plugin: Parent is despawning. Queueing recursive child deletion!");
            while let Some(child_handle) = parent_comp.children.pop() {
                commands
                    .entity(child_handle.clone())
                    .remove::<ChildOf>()
                    .despawn();
            }
            commands.entity(dead_entity.clone()).remove::<Children>();
        }

        if let Some((child_entity, childof)) = child_query.get(dead_entity) {
            println!("Hierarchy Plugin: Child is dying, Unlinking it from parent");
            #[allow(unused_mut)]
            if let Some(mut parent_comp) = parent_query.get_mut(&childof.parent) {
                parent_comp.children.retain(|entity| entity != child_entity);
            }
        }
    }

    for view in link_query.iter() {
        for (child_entity, link_comp) in view.iter() {
            if commands.will_despawn(&link_comp.parent) {
                println!(
                    "Hierarchy Plugin: Target parent is dying before linkage completes. Stripping LinkTo component safely..."
                );
                commands.entity(child_entity.clone()).remove::<LinkTo>();
            }
        }
    }

    for view in unlink_query.iter() {
        for (child_entity, childof) in view.iter() {
            if commands.will_despawn(&childof.parent) {
                println!(
                    "Hierarchy plugin: Unlink target is dying, removing unlink comp from entity"
                );
                commands
                    .entity(child_entity.clone())
                    .remove::<UnlinkChild>();
            }
        }
    }

    for view in unlink_orphan_query.iter() {
        for entity in view.iter() {
            println!("Entity was set for unlink without parent, removing UnlinkChild component");
            commands.entity(entity.clone()).remove::<UnlinkChild>();
        }
    }
}

pub trait HierarchyEntityCommandsExt {
    fn set_parent(&mut self, new_parent: Entity);
    fn unlink_parent(&mut self);
}

impl<'a, 'b> HierarchyEntityCommandsExt for EntityCommands<'a, 'b> {
    fn set_parent(&mut self, new_parent: Entity) {
        self.insert(LinkTo { parent: new_parent });
    }
    fn unlink_parent(&mut self) {
        self.insert(UnlinkChild);
    }
}

#[derive(Component)]
struct AlphaCommander;
#[derive(Component)]
struct BetaCommander;
#[derive(Component)]
struct MinionSubUnit;

#[derive(Component)]
struct TestOrphanMarker;
#[derive(Component)]
struct AlphaTestParent;
#[derive(Component)]
struct BetaTestChild;

#[derive(Resource)]
pub struct FrameStepper {
    pub current_frame: u32,
}

fn setup_scene_graph(commands: Commands) {
    println!("Setup System: Spawning core base entities...");
    commands.spawn(AlphaCommander);
    commands.spawn(BetaCommander);
    commands.spawn(MinionSubUnit);

    commands.spawn(AlphaTestParent);
    commands.spawn(BetaTestChild);
}

fn build_generation_hierarchy(
    commands: Commands,
    alpha_query: Query<Entity, With<AlphaCommander>>,
    beta_query: Query<Entity, With<BetaCommander>>,
    minion_query: Query<Entity, With<MinionSubUnit>>,
    test_parent_query: Query<Entity, With<AlphaTestParent>>,
    test_child_query: Query<Entity, With<BetaTestChild>>,
) {
    let alpha_ent = alpha_query.single();
    let beta_ent = beta_query.single();
    let minion_ent = minion_query.single();
    let test_parent_ent = test_parent_query.single();
    let test_child_ent = test_child_query.single();

    if let (Some(alpha), Some(beta), Some(minion)) = (alpha_ent, beta_ent, minion_ent) {
        commands.entity(beta.clone()).set_parent(alpha.clone());
        commands.entity(minion.clone()).set_parent(beta.clone());
        commands.entity(alpha.clone()).set_parent(minion.clone());
        println!("Linked hierarchy");
    }

    if let (Some(parent), Some(child)) = (test_parent_ent, test_child_ent) {
        commands.entity(child.clone()).set_parent(parent.clone());
        println!("Linked clean test hierarchy for normal unlink validation");
    }
}

fn trigger_runtime_lifecycle_stages(
    commands: Commands,
    stepper: Res<FrameStepper>,
    alpha_query: Query<Entity, With<AlphaCommander>>,
) {
    let frame = stepper.current_frame;
    if frame == 1 {
        if let Some(alpha_entity) = alpha_query.single() {
            println!(
                "System (Update Frame {}): Despawning root AlphaCommander!",
                frame
            );
            commands.entity(alpha_entity.clone()).despawn();
        }
    }
}

fn test_unlink_edge_cases_system(
    commands: Commands,
    stepper: Res<FrameStepper>,
    beta_query: Query<Entity, With<BetaCommander>>,
    minion_query: Query<Entity, With<MinionSubUnit>>,
    orphan_query: Query<Entity, With<TestOrphanMarker>>,
    test_child_query: Query<Entity, With<BetaTestChild>>,
) {
    let frame = stepper.current_frame;

    if frame == 2 {
        let test_child_ent = test_child_query.single();
        if let Some(child) = test_child_ent {
            println!(
                "Unlink System (Frame {}): Requesting normal unlink_child on BetaTestChild.",
                frame
            );
            commands.entity(child.clone()).unlink_parent();
        }
    }

    if frame == 3 {
        commands.spawn(TestOrphanMarker);
        println!(
            "Unlink System (Frame {}): Spawned a standalone orphan entity with tracking component.",
            frame
        );
    }

    if frame == 4 {
        let orphan_ent = orphan_query.single();
        if let Some(orphan) = orphan_ent {
            println!(
                "Unlink System (Frame {}) [EDGE CASE]: Requesting unlink on an orphan (no parent).",
                frame
            );
            commands.entity(orphan.clone()).unlink_parent();
        }

        let minion_ent = minion_query.single();
        let beta_ent = beta_query.single();

        if let (Some(minion), Some(beta)) = (minion_ent, beta_ent) {
            println!(
                "Unlink System (Frame {}) [EDGE CASE]: Staging a LinkTo and UnlinkChild simultaneously on MinionSubUnit.",
                frame
            );
            commands.entity(minion.clone()).set_parent(beta.clone());
            commands.entity(minion.clone()).unlink_parent();
        }
    }
}

fn run_test_frames(app: &mut App) {
    app.build();
    println!("\nStartup: Spawning Entities and building hierarchy");
    app.run_startup();

    println!("\n--- Frame 1: Trigger Root Despawn ---");
    app.update();

    println!("\n--- Frame 2: Run Nominal Unlink Commands ---");
    app.update();

    println!("\n--- Frame 3: Spawn Free Orphan Node ---");
    app.update();

    println!("\n--- Frame 4: Trigger Edge Case Unlinking & Race Conditions ---");
    app.update();

    println!("\n--- Frame 5: Post-Unlink Boundary Evaluation Phase ---");
    app.update();

    println!("\nSimulation processed entire lifecycle safely without a handle violation panic!");
}

fn increment_frame(mut frame: ResMut<FrameStepper>) {
    frame.current_frame += 1;
}

fn main() {
    println!("--- Starting Hirarchy Example ---");

    App::new()
        .add_plugins(HierarchyPlugin)
        .insert_resource(FrameStepper { current_frame: 0 })
        .add_systems(Startup, (setup_scene_graph, build_generation_hierarchy))
        .add_systems(
            Update,
            (
                increment_frame,
                trigger_runtime_lifecycle_stages,
                test_unlink_edge_cases_system,
            ),
        )
        .set_runner(run_test_frames)
        .run();
}
