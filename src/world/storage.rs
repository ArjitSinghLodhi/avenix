use dashmap::DashSet;
use fxhash::{FxBuildHasher, FxHashMap};
use parking_lot::RwLockWriteGuard;
use std::{
    any::{Any, TypeId, type_name},
    cell::UnsafeCell,
    sync::atomic::{AtomicU8, Ordering},
    thread::{self, ThreadId},
};

#[cfg(feature = "events")]
use crate::events::{Event, EventBuffer, ParallelEventReader, ParallelEventWriter, TRACKED_EVENTS};
use crate::{
    commands::{CommandBuffer, DespawnCommand, ParallelCommands},
    entity::Entity,
    registry::REGISTRY_HANDLE_COUNT,
    resources::Resource,
    world::archetypes::ArchetypeManager,
};

#[cfg(feature = "reactivity")]
use crate::reactivity::{REMOVAL_TRACKED_COMPS, TRACKED_COMPONENTS};

pub(crate) struct CurrentBufferIdx;
static CURRENT_BUFFER_IDX: AtomicU8 = AtomicU8::new(1);

impl CurrentBufferIdx {
    #[inline]
    #[cfg(feature = "reactivity")]
    pub(crate) fn current_read_idx() -> u8 {
        CURRENT_BUFFER_IDX.load(Ordering::Relaxed)
    }
    #[cfg(feature = "reactivity")]
    pub(crate) fn current_write_idx() -> u8 {
        let idx = CURRENT_BUFFER_IDX.load(Ordering::Relaxed);
        if idx == 0 { 1 } else { 0 }
    }

    pub(crate) fn advance() {
        let idx = CURRENT_BUFFER_IDX.load(Ordering::Relaxed);
        let next_idx = if idx == 1 { 0 } else { 1 };
        CURRENT_BUFFER_IDX.store(next_idx, Ordering::Relaxed);
    }
}

pub struct World {
    pub(crate) archetypes_manager: ArchetypeManager,
    pub(crate) resources: FxHashMap<TypeId, UnsafeCell<Box<dyn Any>>>,
    pub(crate) commands: CommandBuffer,
    pub(crate) free_indices_list: Vec<u32>,
    pub(crate) main_thread_id: ThreadId,
}

impl World {
    pub(crate) fn new() -> Self {
        Self {
            archetypes_manager: ArchetypeManager::new(),
            resources: FxHashMap::default(),
            commands: CommandBuffer::new(),
            free_indices_list: Vec::new(),
            main_thread_id: thread::current().id(),
        }
    }

    fn validate_thread_safety<T: Resource>(&self) {
        if thread::current().id() != self.main_thread_id {
            panic!(
                "Thread-safety violation! Cannot access Non-Send resource '{}' from background thread: {:?}. Must be accessed from Main Thread: {:?}",
                type_name::<T>(),
                thread::current().id(),
                self.main_thread_id
            );
        }
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<T>())
    }

    pub fn insert_resource<T: Resource + Send + Sync>(&mut self, resource: T) {
        let type_id = std::any::TypeId::of::<T>();
        let boxed_cell = std::cell::UnsafeCell::new(Box::new(resource) as Box<dyn std::any::Any>);
        self.resources.insert(type_id, boxed_cell);
    }

    pub fn insert_non_send_resource<T: Resource>(&mut self, resource: T) {
        self.validate_thread_safety::<T>();
        let type_id = std::any::TypeId::of::<T>();
        let boxed_cell = std::cell::UnsafeCell::new(Box::new(resource) as Box<dyn std::any::Any>);
        self.resources.insert(type_id, boxed_cell);
    }

    pub fn remove_resource<T: Resource + Send + Sync>(&mut self) -> bool {
        let type_id = TypeId::of::<T>();
        self.resources.remove(&type_id).is_some()
    }

    pub fn remove_non_send_resource<T: Resource>(&mut self) -> bool {
        self.validate_thread_safety::<T>();
        let type_id = TypeId::of::<T>();
        self.resources.remove(&type_id).is_some()
    }

    pub fn get_resource<T: Resource + Send + Sync>(&self) -> &T {
        let type_id = TypeId::of::<T>();
        let cell = self.resources.get(&type_id).unwrap_or_else(|| {
            panic!(
                "Requested resource: '{}' was never registered!",
                type_name::<T>()
            );
        });

        unsafe {
            let base_any = &*cell.get();
            base_any
                .downcast_ref::<T>()
                .expect("Resource type mismatch!")
        }
    }

    pub fn get_non_send_resource<T: Resource>(&self) -> &T {
        self.validate_thread_safety::<T>();
        let type_id = TypeId::of::<T>();
        let cell = self.resources.get(&type_id).unwrap_or_else(|| {
            panic!(
                "Requested non-send resource: '{}' was never registered!",
                type_name::<T>()
            );
        });

        unsafe {
            let base_any = &*cell.get();
            base_any
                .downcast_ref::<T>()
                .expect("Resource type mismatch!")
        }
    }

    pub fn get_resource_mut<T: Resource + Send + Sync>(&mut self) -> &mut T {
        let type_id = TypeId::of::<T>();
        let cell = self.resources.get_mut(&type_id).unwrap_or_else(|| {
            panic!(
                "Requested resource: '{}' was never registered!",
                type_name::<T>()
            );
        });
        let base_any = cell.get_mut();
        base_any
            .downcast_mut::<T>()
            .expect("Resource type mismatch!")
    }

    pub fn get_non_send_resource_mut<T: Resource>(&mut self) -> &mut T {
        self.validate_thread_safety::<T>();
        let type_id = TypeId::of::<T>();
        let cell = self.resources.get_mut(&type_id).unwrap_or_else(|| {
            panic!(
                "Requested non-send resource: '{}' was never registered!",
                type_name::<T>()
            );
        });
        let base_any = cell.get_mut();
        base_any
            .downcast_mut::<T>()
            .expect("Resource type mismatch!")
    }

    pub fn get_resource_opt<T: Resource + Send + Sync>(&self) -> Option<&T> {
        let type_id = TypeId::of::<T>();
        let cell = self.resources.get(&type_id)?;

        unsafe {
            let base_any = &*cell.get();
            let casted_ref = base_any.downcast_ref::<T>()?;
            Some(casted_ref)
        }
    }

    pub fn get_non_send_resource_opt<T: Resource>(&self) -> Option<&T> {
        self.validate_thread_safety::<T>();
        let type_id = TypeId::of::<T>();
        let cell = self.resources.get(&type_id)?;

        unsafe {
            let base_any = &*cell.get();
            let casted_ref = base_any.downcast_ref::<T>()?;
            Some(casted_ref)
        }
    }

    pub fn get_resource_mut_opt<T: Resource + Send + Sync>(&mut self) -> Option<&mut T> {
        let type_id = TypeId::of::<T>();
        let cell = self.resources.get_mut(&type_id)?;
        let base_any = cell.get_mut();
        let casted_mut = base_any.downcast_mut::<T>()?;
        Some(casted_mut)
    }

    pub fn get_non_send_resource_mut_opt<T: Resource>(&mut self) -> Option<&mut T> {
        self.validate_thread_safety::<T>();
        let type_id = TypeId::of::<T>();
        let cell = self.resources.get_mut(&type_id)?;
        let base_any = cell.get_mut();
        let casted_mut = base_any.downcast_mut::<T>()?;
        Some(casted_mut)
    }

    pub fn apply_commands(&mut self) {
        self.apply_queue_commands();
        self.apply_despawns();
    }

    pub fn apply_queue_commands(&mut self) {
        let queue_arc = self.commands.queue.clone();
        let mut queue_gaurd = queue_arc.write();
        queue_gaurd.apply(self);
    }

    pub fn apply_despawns(&mut self) {
        let despawns_arc = self.commands.despawns.clone();
        let mut despawns_gaurd = despawns_arc.write();
        apply_despawns(self, &mut despawns_gaurd);
    }

    pub(crate) fn end_of_frame_sync(&mut self) {
        CurrentBufferIdx::advance();
        #[cfg(feature = "reactivity")]
        self.clear_trackers();
        #[cfg(feature = "events")]
        self.clear_events();
    }

    #[cfg(feature = "reactivity")]
    fn clear_trackers(&mut self) {
        use crate::reactivity::REMOVAL_TRACKED_COMPS;
        let tracked_comps = TRACKED_COMPONENTS.read();
        if !tracked_comps.is_empty() {
            for archetype in self.archetypes_manager.archetypes.values_mut() {
                unsafe {
                    let columns = &mut *archetype.columns.get();

                    for meta in tracked_comps.values() {
                        if let Some(marker_column) = columns.get_mut(&meta.marker_id) {
                            let raw_any = marker_column.data.as_any_mut();
                            (meta.clear_column_markers)(raw_any);
                        }
                    }
                }
            }
        }
        let removal_track = REMOVAL_TRACKED_COMPS.read();
        if !removal_track.is_empty() {
            for remove_meta in REMOVAL_TRACKED_COMPS.read().values() {
                (remove_meta.swap_and_clear_buffer)(self)
            }
        }
    }

    #[cfg(feature = "events")]
    fn clear_events(&mut self) {
        let tracked_events = TRACKED_EVENTS.read();
        if tracked_events.is_empty() {
            return;
        }
        for meta in tracked_events.iter() {
            let unsafecell = self
                .resources
                .get_mut(&meta.event_id)
                .expect("Registered event Not initialized somehow? maybe removed");
            (meta.clear_events)(unsafecell);
        }
    }

    /// Returns a thread-safe, thread-clonable [`ParallelCommands`] handle.
    ///
    /// This method can be called directly on the `App` or `World` to obtain a detached,
    /// safe remote input into the engine's command pipeline, manageable by external or
    /// parallel background worker threads.
    pub fn get_par_commands(&mut self) -> ParallelCommands {
        ParallelCommands {
            queue: self.commands.queue.clone(),
            despawns: self.commands.despawns.clone(),
        }
    }

    /// Returns a thread-safe, thread-clonable [`ParallelEventWriter`] handle.
    ///
    /// This method can be called directly on the `App` or `World` to obtain a detached,
    /// safe remote output channel into the engine's event queue, manageable by external
    /// or parallel background worker threads.
    #[cfg(feature = "events")]
    pub fn get_par_event_writer<T: Event>(&mut self) -> ParallelEventWriter<T> {
        ParallelEventWriter {
            write_buffer: self.get_resource::<EventBuffer<T>>().write_queue.clone(),
        }
    }

    /// Returns a thread-safe, thread-clonable [`ParallelEventReader`] handle.
    ///
    /// This method can be called directly on the `App` or `World` to obtain a detached,
    /// safe remote input channel to inspect the engine's event queue from external or
    /// parallel background worker threads.
    #[cfg(feature = "events")]
    pub fn get_par_event_reader<T: Event>(&mut self) -> ParallelEventReader<T> {
        ParallelEventReader {
            read_buffer: self.get_resource::<EventBuffer<T>>().read_queue.clone(),
        }
    }
}

pub(crate) fn apply_despawns(
    world: &mut World,
    despawns_gaurd: &mut RwLockWriteGuard<'_, DashSet<Entity, FxBuildHasher>>,
) {
    #[cfg(feature = "reactivity")]
    if !despawns_gaurd.is_empty() {
        for meta in REMOVAL_TRACKED_COMPS.read().values() {
            (meta.clear_dead_entities)(world, despawns_gaurd)
        }
    }
    for shard in despawns_gaurd.shards() {
        let mut lock = shard.write();
        for (entity_ref, _) in lock.drain() {
            unsafe {
                REGISTRY_HANDLE_COUNT.decrement_handle(entity_ref.registry_index as usize);
            }
            let cmd_ref = unsafe { &*(&entity_ref as *const Entity as *const DespawnCommand) };
            cmd_ref.apply(world);
        }
    }
}
