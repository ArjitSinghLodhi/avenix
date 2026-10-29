pub(crate) mod bundle;
mod command_queue;
pub(crate) mod command_types;
pub(crate) mod concurrent_despawn_set;
mod parallel_commands;
pub(crate) use command_types::DespawnCommand;
pub use parallel_commands::ParallelCommands;
use parking_lot::{RwLock, RwLockReadGuard};
use std::{marker::PhantomData, mem::transmute, sync::Arc};

use crate::{
    commands::{
        bundle::ComponentBundle,
        command_queue::CommandQueue,
        command_types::{
            BatchSpawnCommand, InsertComponentsCommand, RemoveComponentsCommand, SpawnCommand,
        },
        concurrent_despawn_set::ConcurrentBitSet,
    },
    entity::Entity,
    resources::Resource,
    system::{SystemMeta, SystemParam},
    world::World,
};

pub trait Command: 'static + Send {
    fn apply(self, world: &mut World);
}

pub(crate) struct CommandBuffer {
    pub(crate) queue: Arc<RwLock<CommandQueue>>,
    pub(crate) despawns: Arc<ConcurrentBitSet>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(RwLock::new(CommandQueue::new())),
            despawns: Arc::new(ConcurrentBitSet::new()),
        }
    }
}

pub struct Commands<'a> {
    pub(crate) queue: RwLockReadGuard<'a, CommandQueue>,
    pub(crate) despawns: Arc<ConcurrentBitSet>,
}

impl<'a> Commands<'a> {
    pub fn queue<C: Command>(&self, command: C) {
        self.queue.queue(command);
    }

    /// Schedules a command to spawn a single new entity with the provided bundle of components.
    ///
    /// Because this command is deferred, no `Entity` identifier is returned immediately. The
    /// entity is generated and populated during the command execution phase.
    pub fn spawn<C: ComponentBundle>(&self, components: C) {
        self.queue(SpawnCommand { components });
    }

    /// Schedules a command to batch-spawn multiple entities from an iterator of component bundles.
    ///
    /// This is significantly more efficient than calling `.spawn()` multiple times in a loop.
    pub fn spawn_batch<C, I>(&self, components_iter: I)
    where
        C: ComponentBundle + Send,
        I: Iterator<Item = C> + Send + 'static,
    {
        self.queue(BatchSpawnCommand { components_iter });
    }

    pub fn entity<'b>(&'b self, entity: &'b Entity) -> EntityCommands<'a, 'b> {
        EntityCommands {
            entity_id: entity,
            commands: self,
        }
    }

    /// Schedules a command to insert a global resource into the world.
    ///
    /// This command transfers ownership of the resource to the world during the command execution
    /// phase. If a resource of type `T` already exists, it is unconditionally dropped and replaced.
    pub fn insert_resource<T: Resource + Send + Sync>(&self, resource: T) {
        self.queue(|world: &mut World| {
            world.insert_resource(resource);
        });
    }

    /// Schedules a command to remove a global resource of type `T` from the world.
    ///
    /// The resource is dropped during the command execution phase. If the resource does not
    /// exist in the world, the engine handles it gracefully and silently does nothing.
    pub fn remove_resource<T: Resource + Send + Sync>(&self) {
        self.queue(|world: &mut World| {
            world.remove_resource::<T>();
        });
    }
}

pub struct EntityCommands<'a, 'b> {
    entity_id: &'b Entity,
    commands: &'b Commands<'a>,
}

impl<'a, 'b> EntityCommands<'a, 'b> {
    /// Schedules a command to insert a bundle of components onto an entity, overwriting any existing data.
    ///
    /// # Behavior
    ///
    /// * **If the component does not exist:** It is added to the entity. Both [`Added<T>`] and [`Changed<T>`] are flagged.
    /// * **If the component already exists:** The existing value is unconditionally dropped and replaced by the new data. [`Changed<T>`] is flagged
    ///   but [`Added<T>`] is *not*.
    ///
    /// [`Added<T>`]: crate::reactivity::added::Added
    /// [`Changed<T>`]: crate::reactivity::changed::Changed
    pub fn insert<C: ComponentBundle>(&self, components: C) -> &Self {
        self.commands.queue(InsertComponentsCommand {
            entity: self.entity_id.clone(),
            components,
        });
        self
    }

    /// Schedules a command to remove a bundle of components from an entity.
    ///
    /// # Behavior
    ///
    /// The specified component types are queued for removal and will be removed from the entity
    /// during command execution. If a component type in the bundle is not present on
    /// the entity, the engine handles it gracefully and silently does nothing.
    pub fn remove<C: ComponentBundle>(&self) -> &Self {
        self.commands.queue(RemoveComponentsCommand::<C> {
            entity: self.entity_id.clone(),
            _marker: PhantomData,
        });
        self
    }

    /// Schedules a command to despawn the target entity.
    ///
    /// This command is queued and processed later during the command execution phase.
    ///
    /// # Rules
    ///
    /// The engine panics during the despawn command execution phase if any cloned handles referencing this entity
    /// are still alive in memory when the queued despawn command is applied.
    ///
    /// The panic message will display a list of component type_names the entity had right before despawning,
    /// allowing you to instantly identify which entity type caused the violation.
    ///
    /// Look into the documentation of the [`Component`] trait to understand how to use the
    /// [`on_remove`] lifecycle hook to safely clear lingering handle references and satisfy this requirement.
    ///
    /// [`Component`]: crate::component::Component
    /// [`on_remove`]: crate::component::Component::on_remove
    pub fn despawn(&self) -> &Self {
        self.commands.despawns.insert(self.entity_id.clone());
        self
    }

    pub fn entity_id(&self) -> &Entity {
        self.entity_id
    }
}

impl<'a> SystemParam for Commands<'a> {
    type Item<'world> = Commands<'world>;

    fn init_access(_system_meta: &mut SystemMeta) {}

    fn get_param<'world>(world: &'world World) -> Option<Self::Item<'world>> {
        let queue_local = world.commands.queue.read();
        let despawns = world.commands.despawns.clone();
        unsafe {
            let queue = transmute::<
                RwLockReadGuard<'_, CommandQueue>,
                RwLockReadGuard<'world, CommandQueue>,
            >(queue_local);
            Some(Commands { queue, despawns })
        }
    }
}
