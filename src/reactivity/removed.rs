use std::{
    any::{TypeId, type_name},
    marker::PhantomData,
    sync::Arc,
};

use indexmap::IndexMap;
use parking_lot::Mutex;
use rustc_hash::FxBuildHasher;

use crate::{
    commands::concurrent_despawn_set::ConcurrentBitSet,
    ecs::component::Component,
    entity::Entity,
    reactivity::verify_app_not_built_yet,
    resources::{Res, Resource},
    system::{SystemMeta, SystemParam},
    world::World,
};

pub(crate) fn register_removal_tracking_buffers(world: &World) {
    for meta in REMOVAL_TRACKED_COMPS.lock().values() {
        (meta.register_buffer)(world)
    }
}

pub(crate) struct RemovalTrackedMeta {
    pub(crate) register_buffer: fn(&World),
    pub(crate) swap_and_clear_buffer: fn(&mut World),
    pub(crate) clear_dead_entities: fn(&mut World, &Arc<ConcurrentBitSet>),

    pub(crate) push_to_write_queue: fn(&mut World, Entity),
}

pub(crate) static REMOVAL_TRACKED_COMPS: Mutex<
    IndexMap<TypeId, RemovalTrackedMeta, FxBuildHasher>,
> = Mutex::new(IndexMap::with_hasher(FxBuildHasher));

fn register_removal_tracking_comp<T: Component>() {
    let mut tracked = REMOVAL_TRACKED_COMPS.lock();
    let component_id = TypeId::of::<T>();
    if tracked.contains_key(&component_id) {
        return;
    }
    let _lock = verify_app_not_built_yet(type_name::<RemovedComponents<T>>());
    tracked.insert(
        component_id,
        RemovalTrackedMeta {
            register_buffer: |world| {
                world.insert_resource(RemovedComponentsBuffer::<T> {
                    read_queue: Vec::new(),
                    write_queue: Vec::new(),
                    _marker: PhantomData,
                });
            },
            swap_and_clear_buffer: |world| {
                let mut removed_tracking_buffer =
                    world.resource_mut::<RemovedComponentsBuffer<T>>();
                removed_tracking_buffer.read_queue.clear();
                let RemovedComponentsBuffer {
                    read_queue,
                    write_queue,
                    ..
                } = &mut *removed_tracking_buffer;
                std::mem::swap(read_queue, write_queue);
            },
            clear_dead_entities: |world, despawn_guard| {
                let mut removed_tracking_buffer =
                    world.resource_mut::<RemovedComponentsBuffer<T>>();
                removed_tracking_buffer
                    .read_queue
                    .retain(|entity| !despawn_guard.contains(entity));
                removed_tracking_buffer
                    .write_queue
                    .retain(|entity| !despawn_guard.contains(entity));
            },
            push_to_write_queue: |world, entity| {
                let mut buffer = world.resource_mut::<RemovedComponentsBuffer<T>>();
                buffer.write_queue.push(entity);
            },
        },
    );
}

pub struct RemovedComponentsBuffer<T: Component> {
    read_queue: Vec<Entity>,
    pub(crate) write_queue: Vec<Entity>,
    _marker: PhantomData<T>,
}

impl<T: Component> RemovedComponentsBuffer<T> {
    pub fn entities(&self) -> &[Entity] {
        &self.read_queue
    }
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
/// To handle cleanup operations or respond to entities right before they are despawned from the world,
/// look into the documentation of the [`crate::component::Component`] trait and implement the
/// [`crate::component::Component::on_remove`] lifecycle hook.
pub struct RemovedComponents<'w, T: Component> {
    buffer: Res<'w, RemovedComponentsBuffer<T>>,
    _marker: PhantomData<T>,
}

impl<'w, T: Component> RemovedComponents<'w, T> {
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.buffer.read_queue.iter()
    }
}

impl<'w, T: Component> SystemParam for RemovedComponents<'w, T> {
    type Item<'world> = RemovedComponents<'world, T>;

    fn init_access(system_meta: &mut SystemMeta) {
        system_meta.add_resource_read(TypeId::of::<RemovedComponentsBuffer<T>>());
        register_removal_tracking_comp::<T>();
    }

    fn get_param<'world>(world: &'world World) -> Option<Self::Item<'world>> {
        let removed_comp_buffer = world.get_resource::<RemovedComponentsBuffer<T>>()?;

        Some(RemovedComponents {
            buffer: removed_comp_buffer,
            _marker: PhantomData,
        })
    }
}
