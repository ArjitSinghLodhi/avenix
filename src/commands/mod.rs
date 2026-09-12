pub mod bundle;
mod command_queue;
mod command_types;
mod parallel_commands;
pub use command_types::DespawnCommand;
pub use parallel_commands::ParallelCommands;

use std::{marker::PhantomData, sync::Arc};

use dashmap::DashSet;
use fxhash::FxBuildHasher;
use parking_lot::{RawRwLock, RwLock, lock_api::RwLockReadGuard};

use crate::{
    commands::{
        bundle::ComponentBundle,
        command_queue::CommandQueue,
        command_types::{
            AddComponentsCommand, BatchSpawnCommand, InsertComponentsCommand,
            RemoveComponentsCommand, SpawnCommand,
        },
    },
    entity::Entity,
    extensions::SystemMeta,
    system::SystemParam,
    world::storage::World,
};

pub(crate) trait WorldCommand: 'static + Send {
    fn apply(self, world: &mut World);
}

pub(crate) struct CommandBuffer {
    pub(crate) queue: Arc<RwLock<CommandQueue>>,
    pub(crate) despawns: Arc<DashSet<Entity, FxBuildHasher>>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(RwLock::new(CommandQueue::new())),
            despawns: Arc::new(DashSet::with_hasher(FxBuildHasher::new())),
        }
    }
}

pub struct Commands<'a> {
    pub(crate) queue: parking_lot::RwLockReadGuard<'a, CommandQueue>,
    pub(crate) despawns: Arc<DashSet<Entity, FxBuildHasher>>,
}

impl Commands<'_> {
    fn push<C: WorldCommand + 'static>(&mut self, command: C) {
        self.queue.push(command);
    }

    /// Schedules a command to spawn a single new entity with the provided bundle of components.
    ///
    /// Because this command is deferred, no `Entity` identifier is returned immediately. The
    /// entity is generated and populated during the command execution phase.
    pub fn spawn<C: ComponentBundle + Send>(&mut self, components: C) {
        self.push(SpawnCommand { components });
    }

    /// Schedules a command to batch-spawn multiple entities from an iterator of component bundles.
    ///
    /// This is significantly more efficient than calling `.spawn()` multiple times in a loop.
    pub fn spawn_batch<C, I>(&mut self, components_iter: I)
    where
        C: ComponentBundle + Send,
        I: IntoIterator<Item = C>,
        I::IntoIter: Send + 'static,
    {
        self.push(BatchSpawnCommand {
            components_iter: Box::new(components_iter.into_iter()),
        });
    }

    /// Schedules a command to despawn the target entity.
    ///
    /// This command is queued and processed later during the command execution phase.
    ///
    /// # Panics
    ///
    /// The engine panics during the despawn command execution phase if any cloned handles referencing this entity
    /// are still active when the queued despawn command is applied.
    ///
    /// The panic message will display a `HashSet` containing the `std::any::type_name`
    /// of every component within the entity's archetype, allowing you to instantly
    /// identify which entity type caused the violation.
    ///
    /// For projects using the [`DefaultSchedulesPlugin`], look into its documentation to understand
    /// how some schedules are deliberately structured to help you use `despawn_iter` and
    /// `will_despawn` to satisfy this requirement.
    ///
    /// [`DefaultSchedulesPlugin`]: crate::schedule::DefaultSchedulesPlugin
    pub fn despawn(&mut self, entity: Entity) {
        self.despawns.insert(entity);
    }

    /// Schedules a command to add a bundle of components to an entity without overwriting existing data.
    ///
    /// # Behavior
    ///
    /// * **If the component does not exist:** It is added to the entity. The `Added<T>` filter is flagged as
    ///   `true`, but the `Changed<T>` filter is **not** notified.
    /// * **If the component already exists:** The command is ignored and the data is **not** overwritten.
    ///   Neither the `Added<T>` nor `Changed<T>` filters are notified.
    pub fn add_components<C: ComponentBundle + Send>(&mut self, entity: Entity, components: C) {
        self.push(AddComponentsCommand { entity, components });
    }

    /// Schedules a command to insert a bundle of components onto an entity, overwriting any existing data.
    ///
    /// # Behavior
    ///
    /// * **If the component does not exist:** It is added to the entity. The `Added<T>` filter is flagged as
    ///   `true`, but the `Changed<T>` filter is **not** notified.
    /// * **If the component already exists:** The existing value is unconditionally dropped and replaced by the new data. Neither the
    ///   `Added<T>` nor `Changed<T>` filters are notified.
    pub fn insert_components<C: ComponentBundle + Send>(&mut self, entity: Entity, components: C) {
        self.push(InsertComponentsCommand { entity, components });
    }

    /// Schedules a command to remove a bundle of components from an entity.
    ///
    /// # Behavior
    ///
    /// The specified component types are queued for removal and will be removed from the entity
    /// during command execution. If a component type in the bundle is not present on
    /// the entity, the engine handles it gracefully and silently does nothing.
    pub fn remove_components<C: ComponentBundle + Send>(&mut self, entity: Entity) {
        self.push(RemoveComponentsCommand::<C> {
            entity,
            _marker: PhantomData,
        });
    }

    /// Returns an iterator over all currently queued despawn commands.
    ///
    /// This allows you to inspect which entities are scheduled for removal. You can pass
    /// the entity retrieved from [`despawn_target()`] into your query lookup functions to safely
    /// access and drop any active cloned handles.
    ///
    /// See [`DefaultSchedulesPlugin`] to understand how the engine's schedules are structured
    /// to coordinate this iterator with handle cleanup requirements.
    ///
    /// [`despawn_target()`]: crate::commands::command_types::DespawnCommand::despawn_target
    /// [`DefaultSchedulesPlugin`]: crate::schedule::DefaultSchedulesPlugin
    pub fn despawn_iter(&self) -> impl Iterator<Item = &DespawnCommand> {
        self.despawns.iter().map(|entity_ref| unsafe {
            &*(&(*entity_ref) as *const Entity as *const DespawnCommand)
        })
    }

    /// Returns whether the specified entity is currently scheduled for removal.
    ///
    /// This performs an `O(1)` lookup via an internal `HashSet::contains` check, making it
    /// highly efficient to call within heavy system loops. Use this check to conditionally
    /// bypass logic or drop active cloned handles via query lookup functions before the
    /// cleanup phase completes.
    ///
    /// See [`DefaultSchedulesPlugin`] to understand how the engine's schedules are structured
    /// to coordinate this check with handle cleanup requirements.
    ///
    /// [`DefaultSchedulesPlugin`]: crate::schedule::DefaultSchedulesPlugin
    pub fn will_despawn(&self, entity: &Entity) -> bool {
        self.despawns.contains(entity)
    }

    pub(crate) fn push_fn<F>(&mut self, f: F)
    where
        F: FnOnce(&mut World) + Send + 'static,
    {
        self.queue.push_fn(f);
    }

    /// Schedules a command to insert a global resource into the world.
    ///
    /// This command transfers ownership of the resource to the world during the command execution
    /// phase. If a resource of type `T` already exists, it is unconditionally dropped and replaced.
    pub fn insert_resource<T: 'static + Send>(&mut self, resource: T) {
        self.push_fn(|world| world.insert_resource(resource));
    }

    /// Schedules a command to remove a global resource of type `T` from the world.
    ///
    /// The resource is dropped during the command execution phase. If the resource does not
    /// exist in the world, the engine handles it gracefully and silently does nothing.
    pub fn remove_resource<T: 'static + Send>(&mut self) {
        self.push_fn(|world| {
            world.remove_resource::<T>();
        });
    }
}

impl<'a> SystemParam for Commands<'a> {
    fn init_access(_system_meta: &mut SystemMeta) {}
    fn get_param(world: &mut World) -> Self {
        let queue_local = world.commands.queue.read();
        let despawns_arc = world.commands.despawns.clone();

        unsafe {
            let queue = std::mem::transmute::<
                RwLockReadGuard<'_, RawRwLock, CommandQueue>,
                RwLockReadGuard<'_, RawRwLock, CommandQueue>,
            >(queue_local);
            Self {
                queue,
                despawns: despawns_arc,
            }
        }
    }
}
