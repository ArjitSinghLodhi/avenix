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

#[derive(SystemSet, Clone, PartialEq, Debug)]
enum HierarchySet {
    UpdateSync,
    Cleanup,
}

impl Plugin for HierarchyPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, HierarchySet::UpdateSync)
            .configure_sets(CleanupHandles, HierarchySet::Cleanup)
            .add_systems(
                Update,
                (hierarchy_linker_system, hierarchy_unlinker_system)
                    .in_set(HierarchySet::UpdateSync),
            )
            .add_systems(
                CleanupHandles,
                (
                    hirearchy_cleanup_despawns,
                    hierarchy_cleanup_dead_link_entities,
                    hierarchy_cleanup_dead_unlink_entities,
                    hierarchy_cleanup_orphan_unlink_entities,
                )
                    .in_set(HierarchySet::Cleanup),
            );
    }
}

fn hierarchy_linker_system(
    commands: Commands,
    mut parent_query: Query<&mut Children>,
    link_query: Query<(Entity, &LinkTo)>,
) {
    for (child_entity, link_comp) in link_query.iter() {
        let parent_target = &link_comp.parent;
        #[allow(unused_mut)]
        if let Some(mut parent_comp) = parent_query.get_mut(&parent_target) {
            parent_comp.children.push(child_entity.clone());
        } else {
            commands.entity(parent_target).add(Children {
                children: vec![child_entity.clone()],
            });
        }
        commands
            .entity(child_entity)
            .insert(ChildOf {
                parent: parent_target.clone(),
            })
            .remove::<LinkTo>();
    }
}

fn hierarchy_unlinker_system(
    commands: Commands,
    mut parent_query: Query<&mut Children>,
    unlink_query: Query<(Entity, &ChildOf), With<UnlinkChild>>,
) {
    for (child_entity, child_of) in unlink_query.iter() {
        #[allow(unused_mut)]
        if let Some(mut parent_comp) = parent_query.get_mut(&child_of.parent) {
            parent_comp.children.retain(|entity| entity != child_entity);
            println!("Hierarchy Plugin: Unlinked child from parent");
            if parent_comp.children.is_empty() {
                println!(
                    "Hierarchy Plugin: Removing Children component from parent because there are no children anymore"
                );
                commands.entity(&child_of.parent).remove::<Children>();
            }
        }
        commands
            .entity(child_entity)
            .remove::<(UnlinkChild, ChildOf)>();
    }
}

fn hirearchy_cleanup_despawns(
    commands: Commands,
    mut parent_query: Query<&mut Children>,
    child_query: Query<(Entity, &ChildOf)>,
) {
    commands.for_each_despawn(|dead_entity| {
        #[allow(unused_mut)]
        if let Some(mut parent_comp) = parent_query.get_mut(dead_entity) {
            println!("Hierarchy Plugin: Parent is despawning. Queueing recursive child deletion!");
            while let Some(child_handle) = parent_comp.children.pop() {
                commands.entity(&child_handle).remove::<ChildOf>().despawn();
            }
            commands.entity(dead_entity).remove::<Children>();
        }

        if let Some((child_entity, childof)) = child_query.get(dead_entity) {
            println!("Hierarchy Plugin: Child is dying, Unlinking it from parent");
            #[allow(unused_mut)]
            if let Some(mut parent_comp) = parent_query.get_mut(&childof.parent) {
                parent_comp.children.retain(|entity| entity != child_entity);
            }
        }
    });
}

fn hierarchy_cleanup_dead_link_entities(commands: Commands, link_query: Query<(Entity, &LinkTo)>) {
    for (child_entity, link_comp) in link_query.iter() {
        if commands.will_despawn(&link_comp.parent) {
            println!(
                "Hierarchy Plugin: Target parent is dying before linkage completes. Stripping LinkTo component safely..."
            );
            commands.entity(child_entity).remove::<LinkTo>();
        }
    }
}

fn hierarchy_cleanup_dead_unlink_entities(
    commands: Commands,
    unlink_query: Query<(Entity, &ChildOf), With<UnlinkChild>>,
) {
    for (child_entity, childof) in unlink_query.iter() {
        if commands.will_despawn(&childof.parent) {
            println!("Hierarchy plugin: Unlink target is dying, removing unlink comp from entity");
            commands.entity(child_entity).remove::<UnlinkChild>();
        }
    }
}

fn hierarchy_cleanup_orphan_unlink_entities(
    commands: Commands,
    unlink_orphan_query: Query<Entity, (With<UnlinkChild>, Without<ChildOf>)>,
) {
    for entity in unlink_orphan_query.iter() {
        println!("Entity was set for unlink without parent, removing UnlinkChild component");
        commands.entity(entity).remove::<UnlinkChild>();
    }
}

trait HierarchyEntityCommandsExt {
    fn set_parent(&self, new_parent: Entity) -> &Self;
    fn remove_parent(&self) -> &Self;
}

impl<'a, 'b> HierarchyEntityCommandsExt for EntityCommands<'a, 'b> {
    fn set_parent(&self, new_parent: Entity) -> &Self {
        self.insert(LinkTo { parent: new_parent });
        self
    }
    fn remove_parent(&self) -> &Self {
        self.insert(UnlinkChild);
        self
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
        commands.entity(beta).set_parent(alpha.clone());
        commands.entity(minion).set_parent(beta.clone());
        commands.entity(alpha).set_parent(minion.clone());
        println!("Linked hierarchy");
    }

    if let (Some(parent), Some(child)) = (test_parent_ent, test_child_ent) {
        commands.entity(child).set_parent(parent.clone());
        println!("Linked clean test hierarchy for normal unlink validation");
    }
}

fn trigger_runtime_lifecycle_stage_1(
    commands: Commands,
    alpha_query: Query<Entity, With<AlphaCommander>>,
) {
    if let Some(alpha_entity) = alpha_query.single() {
        println!("System (Update Frame 1): Despawning root AlphaCommander!");
        commands.entity(alpha_entity).despawn();
    }
}

fn test_unlink_edge_cases_frame_2(
    commands: Commands,
    test_child_query: Query<Entity, With<BetaTestChild>>,
) {
    let test_child_ent = test_child_query.single();
    if let Some(child) = test_child_ent {
        println!("Unlink System (Frame 2): Requesting normal unlink_child on BetaTestChild.");
        commands.entity(child).remove_parent();
    }
}

fn test_unlink_edge_cases_frame_3(commands: Commands) {
    commands.spawn(TestOrphanMarker);
    println!(
        "Unlink System (Frame 3): Spawned a standalone orphan entity with tracking component."
    );
}

fn test_unlink_edge_cases_frame_4(
    commands: Commands,
    beta_query: Query<Entity, With<BetaCommander>>,
    minion_query: Query<Entity, With<MinionSubUnit>>,
    orphan_query: Query<Entity, With<TestOrphanMarker>>,
) {
    let orphan_ent = orphan_query.single();
    if let Some(orphan) = orphan_ent {
        println!(
            "Unlink System (Frame 4) [EDGE CASE]: Requesting unlink on an orphan (no parent)."
        );
        commands.entity(orphan).remove_parent();
    }

    let minion_ent = minion_query.single();
    let beta_ent = beta_query.single();

    if let (Some(minion), Some(beta)) = (minion_ent, beta_ent) {
        println!(
            "Unlink System (Frame 4) [EDGE CASE]: Staging a LinkTo and UnlinkChild simultaneously on MinionSubUnit."
        );
        commands
            .entity(minion)
            .set_parent(beta.clone())
            .remove_parent();
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
                trigger_runtime_lifecycle_stage_1.run_if(
                    |stepper: &ParallelResourceAccessor<FrameStepper>| {
                        stepper.scope(|s| s.current_frame == 1)
                    },
                ),
                test_unlink_edge_cases_frame_2.run_if(
                    |stepper: &ParallelResourceAccessor<FrameStepper>| {
                        stepper.scope(|s| s.current_frame == 2)
                    },
                ),
                test_unlink_edge_cases_frame_3.run_if(
                    |stepper: &ParallelResourceAccessor<FrameStepper>| {
                        stepper.scope(|s| s.current_frame == 3)
                    },
                ),
                test_unlink_edge_cases_frame_4.run_if(
                    |stepper: &ParallelResourceAccessor<FrameStepper>| {
                        stepper.scope(|s| s.current_frame == 4)
                    },
                ),
            )
                .chain()
                .after(HierarchySet::UpdateSync),
        )
        .set_runner(run_test_frames)
        .run();
}
