#![allow(dead_code)]
#![allow(unused_variables)]

use avenix::prelude::*;

#[derive(Component)]
#[component(on_add = LinkTo::handle_link, on_insert = Self::handle_link)]
pub struct LinkTo {
    pub parent: Entity,
}

impl LinkTo {
    pub fn handle_link(world: &mut DeferredWorld, entity: &Entity) {
        let mut parent_target = None;
        {
            if let Some(link) = world.get::<LinkTo>(entity) {
                parent_target = Some(link.parent.clone());
            }
        }

        if let Some(parent) = parent_target {
            let mut appended_to_parent = false;
            {
                if let Some(mut parent_children) = world.get_mut::<Children>(&parent) {
                    parent_children.children.push(entity.clone());
                    appended_to_parent = true;
                }
            }

            if !appended_to_parent {
                world.insert_components(
                    parent.clone(),
                    Children {
                        children: vec![entity.clone()],
                    },
                );
            }

            world.insert_components(entity.clone(), ChildOf { parent });
            world.remove_components::<LinkTo>(entity.clone());
        }
    }
}

#[derive(Component)]
#[component(on_add, on_insert)]
pub struct UnlinkChild;

impl UnlinkChild {
    pub fn on_add(world: &mut DeferredWorld, entity: &Entity) {
        let mut parent_target = None;
        {
            if let Some(child_of) = world.get::<ChildOf>(entity) {
                parent_target = Some(child_of.parent.clone());
            }
        }

        if let Some(parent) = parent_target {
            let mut clean_parent_component = false;
            {
                if let Some(mut parent_children) = world.get_mut::<Children>(&parent) {
                    parent_children.children.retain(|e| e != entity);
                    if parent_children.children.is_empty() {
                        clean_parent_component = true;
                    }
                }
            }

            if clean_parent_component {
                world.remove_components::<Children>(parent);
            }
        }

        world.remove_components::<ChildOf>(entity.clone());
        world.remove_components::<UnlinkChild>(entity.clone());
    }

    pub fn on_insert(world: &mut DeferredWorld, entity: &Entity) {
        Self::on_add(world, entity);
    }
}

#[derive(Component)]
#[component(on_remove = Self::handle_remove)]
pub struct Children {
    pub children: Vec<Entity>,
}

impl Children {
    pub fn handle_remove(world: &mut DeferredWorld, entity: &Entity) {
        let mut children_to_clear = Vec::new();
        {
            if let Some(parent_children) = world.get::<Children>(entity) {
                children_to_clear = parent_children.children.clone();
            }
        }

        for child in children_to_clear {
            let mut child_still_exists = false;
            {
                if world.get::<ChildOf>(&child).is_some() {
                    child_still_exists = true;
                }
            }
            if child_still_exists {
                world.remove_components::<ChildOf>(child);
            }
        }
    }
}

#[derive(Component)]
#[component(on_remove)]
pub struct ChildOf {
    pub parent: Entity,
}

impl ChildOf {
    pub fn on_remove(world: &mut DeferredWorld, entity: &Entity) {
        let mut parent_target = None;
        {
            if let Some(child_of) = world.get::<ChildOf>(entity) {
                parent_target = Some(child_of.parent.clone());
            }
        }

        if let Some(parent) = parent_target {
            let mut clean_parent_component = false;
            let mut parent_still_exists = false;
            {
                if let Some(mut parent_children) = world.get_mut::<Children>(&parent) {
                    parent_still_exists = true;
                    parent_children.children.retain(|e| e != entity);
                    if parent_children.children.is_empty() {
                        clean_parent_component = true;
                    }
                }
            }

            if parent_still_exists && clean_parent_component {
                world.remove_components::<Children>(parent);
            }
        }
    }
}

pub trait HierarchyEntityCommandsExt {
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
    alpha_entity: Single<Entity, With<AlphaCommander>>,
    beta_entity: Single<Entity, With<BetaCommander>>,
    minion_entity: Single<Entity, With<MinionSubUnit>>,
    test_parent_entity: Single<Entity, With<AlphaTestParent>>,
    test_child_entity: Single<Entity, With<BetaTestChild>>,
) {
    commands
        .entity(&beta_entity)
        .set_parent(alpha_entity.clone());
    commands
        .entity(&minion_entity)
        .set_parent(beta_entity.clone());
    commands
        .entity(&alpha_entity)
        .set_parent(minion_entity.clone());
    println!("Linked hierarchy");

    commands
        .entity(&test_child_entity)
        .set_parent(test_parent_entity.clone());
    println!("Linked clean test hierarchy for normal unlink validation");
}

fn trigger_runtime_lifecycle_stage_1(
    commands: Commands,
    alpha_entity: Single<Entity, With<AlphaCommander>>,
) {
    println!("System (Update Frame 1): Despawning root AlphaCommander!");
    commands.entity(&alpha_entity).despawn();
}

fn test_unlink_edge_cases_frame_2(
    commands: Commands,
    test_child_entity: Single<Entity, With<BetaTestChild>>,
) {
    println!("Unlink System (Frame 2): Requesting normal unlink_child on BetaTestChild.");
    commands.entity(&test_child_entity).remove_parent();
}

fn test_unlink_edge_cases_frame_3(commands: Commands) {
    commands.spawn(TestOrphanMarker);
    println!(
        "Unlink System (Frame 3): Spawned a standalone orphan entity with tracking component."
    );
}

fn test_unlink_edge_cases_frame_4(
    commands: Commands,
    beta_entity: Single<Entity, With<BetaCommander>>,
    minion_entity: Single<Entity, With<MinionSubUnit>>,
    orphan_entity: Single<Entity, With<TestOrphanMarker>>,
) {
    println!("Unlink System (Frame 4) [EDGE CASE]: Requesting unlink on an orphan (no parent).");
    commands.entity(&orphan_entity).remove_parent();

    println!(
        "Unlink System (Frame 4) [EDGE CASE]: Staging a LinkTo and UnlinkChild simultaneously on MinionSubUnit."
    );
    commands
        .entity(&minion_entity)
        .set_parent(beta_entity.clone())
        .remove_parent();
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
        .insert_resource(FrameStepper { current_frame: 0 })
        .add_systems(
            Startup,
            (setup_scene_graph, build_generation_hierarchy).chain(),
        )
        .add_systems(
            Update,
            (
                increment_frame,
                trigger_runtime_lifecycle_stage_1.run_if(
                    |stepper: ParallelResourceAccessor<FrameStepper>| {
                        stepper.scope(|s| s.current_frame == 1)
                    },
                ),
                test_unlink_edge_cases_frame_2.run_if(
                    |stepper: ParallelResourceAccessor<FrameStepper>| {
                        stepper.scope(|s| s.current_frame == 2)
                    },
                ),
                test_unlink_edge_cases_frame_3.run_if(
                    |stepper: ParallelResourceAccessor<FrameStepper>| {
                        stepper.scope(|s| s.current_frame == 3)
                    },
                ),
                test_unlink_edge_cases_frame_4.run_if(
                    |stepper: ParallelResourceAccessor<FrameStepper>| {
                        stepper.scope(|s| s.current_frame == 4)
                    },
                ),
            )
                .chain(),
        )
        .set_runner(run_test_frames)
        .run();
}
