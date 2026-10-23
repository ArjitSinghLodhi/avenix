pub(crate) mod archetypes;

use dashmap::DashSet;
use parking_lot::RwLockWriteGuard;
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::{
    any::{Any, TypeId, type_name},
    cell::UnsafeCell,
    marker::PhantomData,
    sync::Arc,
};

use crate::{
    commands::{CommandBuffer, DespawnCommand, ParallelCommands},
    entity::Entity,
    entity_registry::REGISTRY_HANDLE_COUNT,
    resources::{
        ConcurrentResourceRegistry, NonSend, NonSendMut, ParallelResourceAccessor, Res, ResMut,
        Resource,
    },
    states::StateTransitionSystems,
    system::SystemMeta,
    world::archetypes::ArchetypeManager,
};
use crate::{
    events::{Event, ParallelEventReader, ParallelEventWriter, TRACKED_EVENTS},
    query::{QueryData, QueryFilter, parallel_query::ParallelQueryAccessor},
    system::SystemParam,
};

#[cfg(feature = "reactivity")]
use crate::{
    ecs::Component,
    reactivity::{REMOVAL_TRACKED_COMPS, TRACKED_COMPONENTS},
};

pub struct World {
    pub(crate) archetypes_manager: ArchetypeManager,
    pub(crate) resources: Arc<ConcurrentResourceRegistry>,
    pub(crate) non_send_resources: FxHashMap<TypeId, UnsafeCell<Box<dyn Any>>>,
    pub(crate) commands: CommandBuffer,
    pub(crate) free_indices_list: Vec<u32>,
}

unsafe impl Sync for World {}

impl World {
    pub(crate) fn new() -> Self {
        Self {
            archetypes_manager: ArchetypeManager::new(),
            resources: Arc::new(ConcurrentResourceRegistry::new()),
            non_send_resources: FxHashMap::default(),
            commands: CommandBuffer::new(),
            free_indices_list: Vec::new(),
        }
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        self.resources.has_resource::<T>()
    }

    pub fn insert_resource<T: Resource + Send + Sync>(&self, resource: T) -> Option<T> {
        self.resources.insert_resource(resource)
    }

    pub fn insert_non_send_resource<T: Resource>(&mut self, resource: T) -> Option<T> {
        let type_id = std::any::TypeId::of::<T>();
        let boxed_cell = std::cell::UnsafeCell::new(Box::new(resource) as Box<dyn std::any::Any>);
        if let Some(res) = self.non_send_resources.insert(type_id, boxed_cell) {
            let res = res.into_inner().downcast::<T>().unwrap();
            Some(*res)
        } else {
            None
        }
    }

    pub fn remove_resource<T: Resource + Send + Sync>(&self) -> Option<T> {
        self.resources.remove_resource::<T>()
    }

    pub fn remove_non_send_resource<T: Resource>(&mut self) -> Option<T> {
        let type_id = TypeId::of::<T>();
        if let Some(res) = self.non_send_resources.remove(&type_id) {
            let res = res.into_inner().downcast::<T>().unwrap();
            Some(*res)
        } else {
            None
        }
    }

    pub fn resource<'w, T: Resource + Send + Sync>(&'w self) -> Res<'w, T> {
        self.resources.resource::<T>()
    }

    pub fn non_send_resource<'w, T: Resource>(&'w self) -> NonSend<'w, T> {
        let type_id = TypeId::of::<T>();
        let cell = self.non_send_resources.get(&type_id).unwrap_or_else(|| {
            panic!(
                "Requested non-send resource: '{}' was never registered!",
                type_name::<T>()
            );
        });

        unsafe {
            let base_any = &*cell.get();
            let res = base_any
                .downcast_ref::<T>()
                .expect("Resource type mismatch!");
            NonSend {
                val_ptr: res as *const T,
                _marker: PhantomData,
            }
        }
    }

    pub fn resource_mut<'w, T: Resource + Send + Sync>(&'w self) -> ResMut<'w, T> {
        self.resources.resource_mut::<T>()
    }

    pub fn non_send_resource_mut<'w, T: Resource>(&'w mut self) -> NonSendMut<'w, T> {
        let type_id = TypeId::of::<T>();
        let cell = self
            .non_send_resources
            .get_mut(&type_id)
            .unwrap_or_else(|| {
                panic!(
                    "Requested non-send resource: '{}' was never registered!",
                    type_name::<T>()
                );
            });
        let base_any = cell.get_mut();
        let res = base_any
            .downcast_mut::<T>()
            .expect("Resource type mismatch!");
        NonSendMut {
            val_ptr: res as *mut T,
            _marker: PhantomData,
        }
    }

    pub fn get_resource<'w, T: Resource + Send + Sync>(&'w self) -> Option<Res<'w, T>> {
        self.resources.get_resource::<T>()
    }

    pub fn get_non_send_resource<'w, T: Resource>(&'w self) -> Option<NonSend<'w, T>> {
        let type_id = TypeId::of::<T>();
        let cell = self.non_send_resources.get(&type_id)?;

        unsafe {
            let base_any = &*cell.get();
            let casted_ref = base_any.downcast_ref::<T>()?;
            Some(NonSend {
                val_ptr: casted_ref as *const T,
                _marker: PhantomData,
            })
        }
    }

    pub fn get_resource_mut<'w, T: Resource + Send + Sync>(&'w self) -> Option<ResMut<'w, T>> {
        self.resources.get_resource_mut::<T>()
    }

    pub fn get_non_send_resource_mut<'w, T: Resource>(&'w mut self) -> Option<NonSendMut<'w, T>> {
        let type_id = TypeId::of::<T>();
        let cell = self.non_send_resources.get_mut(&type_id)?;
        let base_any = cell.get_mut();
        let casted_mut = base_any.downcast_mut::<T>()?;
        Some(NonSendMut {
            val_ptr: casted_mut as *mut T,
            _marker: PhantomData,
        })
    }

    pub fn apply_queue_commands(&mut self) {
        let queue_arc = self.commands.queue.clone();
        let mut queue_guard = queue_arc.write();
        queue_guard.apply(self);
    }

    pub fn apply_despawns(&mut self) {
        let despawns_arc = self.commands.despawns.clone();
        let mut despawns_guard = despawns_arc.write();
        apply_despawns(self, &mut despawns_guard);
    }

    pub fn end_of_frame_sync(&mut self) {
        #[cfg(feature = "reactivity")]
        self.clear_trackers();
        self.clear_events();
    }

    #[cfg(feature = "reactivity")]
    pub fn register_changed_trackers<T: Component>(&self) {
        crate::reactivity::changed::register_changed_tracked_component::<T>();
    }

    #[cfg(feature = "reactivity")]
    pub fn register_added_trackers<T: Component>(&self) {
        crate::reactivity::added::register_added_tracked_component::<T>();
    }

    #[cfg(feature = "reactivity")]
    pub fn register_removal_trackers<T: Component>(&self) {
        crate::reactivity::removed::register_removal_tracking_buffers(self);
    }

    #[cfg(feature = "reactivity")]
    fn clear_trackers(&mut self) {
        let tracked_comps = TRACKED_COMPONENTS.lock();
        if !tracked_comps.is_empty() {
            for archetype in self.archetypes_manager.archetypes.iter_mut() {
                let archetype_lock = archetype.lock();
                unsafe {
                    for meta_list in tracked_comps.values() {
                        for meta in meta_list {
                            if let Some(marker_column) = archetype_lock.columns.get(&meta.marker_id)
                            {
                                let mut guard = marker_column.data.write();
                                let raw_any = guard.as_any_mut();
                                (meta.clear_column_markers)(raw_any);
                            }
                        }
                    }
                }
            }
        }

        let removal_track = REMOVAL_TRACKED_COMPS.lock();
        if !removal_track.is_empty() {
            for remove_meta in removal_track.values() {
                (remove_meta.swap_and_clear_buffer)(self)
            }
        }
    }

    fn clear_events(&mut self) {
        let tracked_events = TRACKED_EVENTS.read();
        if tracked_events.is_empty() {
            return;
        }
        for meta in tracked_events.iter() {
            let entry = self
                .resources
                .resources
                .get(&meta.event_id)
                .expect("Registered event Not initialized somehow? maybe removed");
            let mut res = entry.value().write();
            (meta.clear_events)(&mut res);
        }
    }

    pub fn start_frame_sync(&mut self) {
        self.transition_states();
    }

    fn transition_states(&mut self) {
        let state_systems_opt = self.get_resource_mut::<StateTransitionSystems>();
        if let Some(mut state_systems) = state_systems_opt {
            state_systems
                .systems
                .iter_mut()
                .for_each(|sys| sys.run(self));
        }
    }

    /// Returns a thread-safe, thread-clonable [`ParallelCommands`] handle.
    ///
    /// This method can be called directly on the `World` to obtain a detached,
    /// safe remote input into the engine's command pipeline, manageable by external or
    /// parallel background worker threads.
    pub fn par_commands(&self) -> ParallelCommands {
        ParallelCommands::get_param(self)
    }

    /// Returns a thread-safe, thread-clonable [`ParallelEventWriter`] handle.
    ///
    /// This method can be called directly on the `World` to obtain a detached,
    /// safe remote output channel into the engine's event queue, manageable by external
    /// or parallel background worker threads.
    pub fn par_event_writer<T: Event>(&self) -> ParallelEventWriter<T> {
        ParallelEventWriter::get_param(self)
    }

    /// Returns a thread-safe, thread-clonable [`ParallelEventReader`] handle.
    ///
    /// This method can be called directly on the `World` to obtain a detached,
    /// safe remote input channel to inspect the engine's event queue from external or
    /// parallel background worker threads.
    pub fn par_event_reader<T: Event>(&self) -> ParallelEventReader<T> {
        ParallelEventReader::get_param(self)
    }

    /// Returns a thread-safe, thread-clonable [`ParallelResourceAccessor`] handle.
    ///
    /// This method can be called directly on the `World` to obtain a detached,
    /// safe remote accessor into the engine's resource tables manageable by external or
    /// parallel background threads.
    pub fn par_resource_accessor<T: Resource + Send + Sync>(&self) -> ParallelResourceAccessor<T> {
        ParallelResourceAccessor::get_param(self)
    }

    pub fn par_query_accessor<Q: QueryData, F: QueryFilter>(&self) -> ParallelQueryAccessor<Q, F> {
        ParallelQueryAccessor::<Q, F>::init_access(&mut SystemMeta::new(
            "par_query_accessor".to_string(),
        ));
        ParallelQueryAccessor::<Q, F>::get_param(self)
    }
}

pub(crate) fn apply_despawns(
    world: &mut World,
    despawns_guard: &mut RwLockWriteGuard<'_, DashSet<Entity, FxBuildHasher>>,
) {
    #[cfg(feature = "reactivity")]
    if !despawns_guard.is_empty() {
        for meta in REMOVAL_TRACKED_COMPS.lock().values() {
            (meta.clear_dead_entities)(world, despawns_guard)
        }
    }
    for shard in despawns_guard.shards() {
        let mut lock = shard.write();
        for (entity, _) in lock.drain() {
            unsafe {
                REGISTRY_HANDLE_COUNT.decrement_handle(entity.registry_index as usize);
            }
            let cmd = DespawnCommand { entity };
            cmd.apply(world);
        }
    }
}

impl World {
    /// # Safety
    ///
    /// The caller must guarantee that:
    /// 1. This method is called exclusively from the **Main Thread** (the thread on which the non-send resource was originally registered and allocated).
    /// 2. No other active references (`&T` or `&mut T`) to this specific resource exist anywhere in the engine for the duration of the returned `NonSendMut` lifetime.
    /// 3. Concurrent execution phases do not attempt to look up or access this resource type asynchronously from background worker pool threads.
    pub unsafe fn non_send_resource_mut_unsafe<'w, T: Resource>(&'w self) -> NonSendMut<'w, T> {
        let type_id = TypeId::of::<T>();
        let cell = self.non_send_resources.get(&type_id).unwrap_or_else(|| {
            panic!(
                "Requested non-send resource: '{}' was never registered!",
                type_name::<T>()
            );
        });
        let base_any = cell.get();
        unsafe {
            let res = (*base_any)
                .downcast_mut::<T>()
                .expect("Resource type mismatch!");
            NonSendMut {
                val_ptr: res as *mut T,
                _marker: PhantomData,
            }
        }
    }

    /// # Safety
    ///
    /// The caller must guarantee that:
    /// 1. This method is called exclusively from the **Main Thread**.
    /// 2. No other active mutable or immutable references to this specific resource exist simultaneously within the current execution scope.
    /// 3. Access is strictly isolated from out-of-band parallel execution workers.
    pub unsafe fn get_non_send_resource_mut_unsafe<'w, T: Resource>(
        &'w self,
    ) -> Option<NonSendMut<'w, T>> {
        let type_id = TypeId::of::<T>();
        let cell = self.non_send_resources.get(&type_id)?;
        let base_any = cell.get();
        let casted_mut = unsafe { (&mut *base_any).downcast_mut::<T>()? };
        Some(NonSendMut {
            val_ptr: casted_mut as *mut T,
            _marker: PhantomData,
        })
    }
}
