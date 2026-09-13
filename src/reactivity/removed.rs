use std::{any::TypeId, marker::PhantomData, sync::Arc};

use dashmap::DashSet;
use fxhash::FxBuildHasher;
use indexmap::IndexMap;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{
    app::App,
    ecs::Component,
    entity::Entity,
    extensions::World,
    resources::Resource,
    system::{SystemMeta, SystemParam},
};

pub(crate) fn register_removal_tracking_buffers(app: &mut App) {
    for meta in REMOVAL_TRACKED_COMPS.read().values() {
        (meta.register_buffer)(app.world_mut())
    }
}

pub(crate) struct RemovalTrackedMeta {
    pub(crate) register_buffer: fn(&mut World),
    pub(crate) swap_and_clear_buffer: fn(&mut World),
    pub(crate) clear_dead_entities: fn(&mut World, &mut RwLockWriteGuard<'_, DashSet<Entity, FxBuildHasher>>),

    pub(crate) push_to_write_queue: fn(&mut World, Entity),
}

pub(crate) static REMOVAL_TRACKED_COMPS: RwLock<
    IndexMap<TypeId, RemovalTrackedMeta, FxBuildHasher>,
> = RwLock::new(IndexMap::with_hasher(FxBuildHasher::new()));

fn register_removal_tracking_comp<T: Component>() {
    let mut tracked = REMOVAL_TRACKED_COMPS.write();
    tracked.insert(
        TypeId::of::<T>(),
        RemovalTrackedMeta {
            register_buffer: |world| {
                world.insert_resource(RemovedComponentsBuffer::<T> {
                    read_queue: Arc::new(RwLock::new(Vec::new())),
                    write_queue: Arc::new(RwLock::new(Vec::new())),
                    _marker: PhantomData,
                });
            },
            swap_and_clear_buffer: |world| {
                let removed_tracking_buffer =
                    world.get_resource_mut::<RemovedComponentsBuffer<T>>();
                let read_queue_gaurd = &mut *removed_tracking_buffer.read_queue.write();
                let write_queue_gaurd = &mut *removed_tracking_buffer.write_queue.write();
                read_queue_gaurd.clear();
                std::mem::swap(read_queue_gaurd, write_queue_gaurd);
            },
            clear_dead_entities: |world, despawn_gaurd| {
                let removed_tracking_buffer =
                    world.get_resource_mut::<RemovedComponentsBuffer<T>>();
                let read_queue_gaurd = &mut *removed_tracking_buffer.read_queue.write();
                let write_queue_gaurd = &mut *removed_tracking_buffer.write_queue.write();
                read_queue_gaurd.retain(|entity| !despawn_gaurd.contains(entity));
                write_queue_gaurd.retain(|entity| !despawn_gaurd.contains(entity));
            },
            push_to_write_queue: |world, entity| {
                let buffer = world.get_resource_mut::<RemovedComponentsBuffer<T>>();
                buffer.write_queue.write().push(entity);
            },
        },
    );
}

pub(crate) struct RemovedComponentsBuffer<T: Component> {
    read_queue: Arc<RwLock<Vec<Entity>>>,
    pub(crate) write_queue: Arc<RwLock<Vec<Entity>>>,
    _marker: PhantomData<T>,
}

impl<T: Component> Resource for RemovedComponentsBuffer<T> {}

/// A system parameter that provides an iterator over entities that had a component of type `T`
/// removed during the previous frame.
///
/// # Architecture & Timing
///
/// Component removal tracking in Avenix is globally double-buffered and strictly time-bound. It does
/// **not** track individual system read history or preserve a permanent ledger. The visibility window
/// follows a strict 3-frame lifecycle:
///
/// * **Frame 1 (Removal):** A component is removed via commands or structural world mutations. The event
///   is captured inside an internal write buffer and is **not** yet visible to systems.
/// * **Frame 2 (Detection Window):** The event becomes globally visible. Any system requesting this
///   `RemovedComponents<T>` parameter during this frame will observe the removed entity handles.
/// * **Frame 3 (Purge):** The tracking state is unconditionally swapped and cleared out of rotation.
///
/// Regardless of whether a system executes or queries the data, the removal event will never last for
/// more than exactly one frame cycle.
///
/// # Dead Entity Pruning
///
/// If an entity has a component removed by despawning, the engine automatically sweeps and prunes that
/// entity's handle from all active removal tracking buffers right as despawn operations are processed.
/// This prevents downstream systems from accidentally iterating over dead or recycled entity IDs.
///
/// To iterate over entities that are scheduled for despawning before they are actually despawned automatically,
/// check if a specific target is doomed via [`crate::commands::Commands::will_despawn`] or list all pending deaths
/// ahead of time using [`crate::commands::Commands::despawn_iter`].
pub struct RemovedComponents<'w, T: Component> {
    read_buffer: RwLockReadGuard<'w, Vec<Entity>>,
    _marker: PhantomData<(&'w (), T)>,
}

impl<'w, T: Component> RemovedComponents<'w, T> {
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.read_buffer.iter()
    }
}

impl<'w, T: Component> SystemParam for RemovedComponents<'w, T> {
    fn init_access(_system_meta: &mut SystemMeta) {
        register_removal_tracking_comp::<T>();
    }
    fn get_param(world: &mut World) -> Self {
        let removed_comp_buffer = world.get_resource_mut::<RemovedComponentsBuffer<T>>();
        let reader = removed_comp_buffer.read_queue.read();
        let reader = unsafe {
            std::mem::transmute::<RwLockReadGuard<Vec<Entity>>, RwLockReadGuard<Vec<Entity>>>(
                reader,
            )
        };
        Self {
            read_buffer: reader,
            _marker: PhantomData,
        }
    }
}
