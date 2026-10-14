pub mod filter;
pub(crate) mod parallel_query;
mod params;

use dashmap::DashMap;
pub use params::Has;

pub use filter::*;

use parking_lot::Mutex;
use rayon::{
    iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator},
    slice::ParallelSlice,
};
use rustc_hash::{FxBuildHasher, FxHashSet};
use std::{marker::PhantomData, sync::Arc};

use crate::{
    entity::Entity,
    entity_registry::REGISTRY,
    extensions::{AccessVec, SystemMeta, SystemParam},
    world::{
        archetypes::{Archetype, ArchetypeId},
        storage::World,
    },
};

#[doc(hidden)]
pub struct ThreadSafe<T> {
    pub(crate) value: T,
}

unsafe impl<T> Send for ThreadSafe<T> {}
unsafe impl<T> Sync for ThreadSafe<T> {}

pub trait QueryData {
    type Item<'w>: Send + Sync;
    type ReadOnlyItem<'w>: Send + Sync;
    type Fetch: Send + Sync;

    fn matches(archetype: &Archetype) -> bool;

    /// # Safety
    ///
    /// Implementations must guarantee that the returned `Self::Fetch` value contains valid,
    /// initialized metadata, pointers, or references that remain valid for the duration of
    /// the borrow layout. It must not lead to out-of-bounds memory access or invalid type casting.
    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch;
    fn collect_access(
        reads: &mut AccessVec<std::any::TypeId>,
        writes: &mut AccessVec<std::any::TypeId>,
    );

    /// # Safety
    ///
    /// * `fetch` must be a valid state originally produced by `init_fetch` for the current archetype.
    /// * `index` must be strictly within the bounds of the allocated entity array for this archetype.
    /// * The caller must ensure that unique, mutable access to the underlying item at `index`
    ///   is maintained globally (no aliasing reads or writes).
    unsafe fn fetch_mut<'w>(fetch: &Self::Fetch, index: usize) -> Self::Item<'w>;

    /// # Safety
    ///
    /// * `fetch` must be a valid state originally produced by `init_fetch`.
    /// * `index` must be strictly within bounds.
    /// * Shared access must be synchronized; no concurrent mutable borrows may exist for this item.
    unsafe fn fetch_read_only<'w>(fetch: &Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w>;
}

pub struct QueryReadOnlyChunk<'b, Q: QueryData> {
    indices: &'b [usize],
    fetch: &'b Q::Fetch,
}

impl<'b, Q: QueryData> QueryReadOnlyChunk<'b, Q> {
    pub fn iter<'c>(&'c self) -> impl Iterator<Item = Q::ReadOnlyItem<'c>> {
        let fetch = self.fetch;
        self.indices
            .iter()
            .map(|&index| unsafe { Q::fetch_read_only(fetch, index) })
    }
}

pub struct QueryMutableChunk<'b, Q: QueryData> {
    indices: &'b [usize],
    fetch: &'b Q::Fetch,
}

impl<'b, Q: QueryData> QueryMutableChunk<'b, Q> {
    pub fn iter<'c>(&'c self) -> impl Iterator<Item = Q::ReadOnlyItem<'c>> {
        let fetch = self.fetch;
        self.indices
            .iter()
            .map(|&index| unsafe { Q::fetch_read_only(fetch, index) })
    }

    pub fn iter_mut<'c>(&'c mut self) -> impl Iterator<Item = Q::Item<'c>> {
        let fetch = self.fetch;
        self.indices
            .iter()
            .map(|&index| unsafe { Q::fetch_mut(fetch, index) })
    }
}

pub struct Query<'q, 'a, Q: QueryData, F: QueryFilter = EmptyQueryFilter> {
    cached_fetches: Vec<Option<Q::Fetch>>,
    cached_indices: Vec<Option<Vec<usize>>>,
    _marker: std::marker::PhantomData<(&'q (), &'a (), F)>,
}

unsafe impl<'q, 'a, Q: QueryData, F: QueryFilter> Send for Query<'q, 'a, Q, F> {}
unsafe impl<'q, 'a, Q: QueryData, F: QueryFilter> Sync for Query<'q, 'a, Q, F> {}

impl<'q, 'a, Q: QueryData, F: QueryFilter> Query<'q, 'a, Q, F> {
    pub(crate) fn new(
        archetypes_map: &'a Arc<DashMap<ArchetypeId, Mutex<Archetype>, FxBuildHasher>>,
    ) -> Self {
        let mut cached_fetches = (0..archetypes_map.len())
            .map(|_| None)
            .collect::<Vec<Option<Q::Fetch>>>();

        let mut cached_indices = vec![None; archetypes_map.len()];

        for arch_mutex in archetypes_map.iter() {
            let arch = arch_mutex.lock();
            let arch_id = arch.id();
            if Q::matches(&arch) && F::matches(&arch) {
                let fetch = unsafe { Q::init_fetch(&arch) };
                let mut indices = (0..arch.entities.len()).collect::<Vec<usize>>();
                let filter_data = F::init_filter_data(&arch);
                indices.retain(|row_idx| F::matches_row(&filter_data, *row_idx));

                cached_fetches[arch_id as usize] = Some(fetch);
                cached_indices[arch_id as usize] = Some(indices);
            }
        }

        Self {
            cached_fetches,
            cached_indices,
            _marker: PhantomData,
        }
    }

    pub fn single<'w>(&'w self) -> Option<Q::ReadOnlyItem<'w>> {
        let mut found_item = None;

        for (arch_id, maybe_indices) in self.cached_indices.iter().enumerate() {
            if let Some(indices) = maybe_indices {
                if indices.is_empty() {
                    continue;
                }

                if found_item.is_some() || indices.len() > 1 {
                    return None;
                }

                let entity_index = indices[0];
                if let Some(fetch_state) = self.cached_fetches[arch_id].as_ref() {
                    unsafe {
                        found_item = Some(Q::fetch_read_only(fetch_state, entity_index));
                    }
                }
            }
        }

        found_item
    }

    pub fn single_mut<'w>(&'w mut self) -> Option<Q::Item<'w>> {
        let mut found_item = None;

        for (arch_id, maybe_indices) in self.cached_indices.iter_mut().enumerate() {
            if let Some(indices) = maybe_indices {
                if indices.is_empty() {
                    continue;
                }

                if found_item.is_some() || indices.len() > 1 {
                    return None;
                }

                let entity_index = indices[0];
                if let Some(fetch_state) = self.cached_fetches[arch_id].as_ref() {
                    unsafe {
                        found_item = Some(Q::fetch_mut(fetch_state, entity_index));
                    }
                }
            }
        }

        found_item
    }

    /// Returns the total number of entity indices across all matching archetypes,
    /// fully accounting for any dynamic filters.
    pub fn total_entities(&self) -> usize {
        self.cached_indices
            .iter()
            .flatten()
            .map(|indices| indices.len())
            .sum()
    }

    pub fn iter<'b>(&'b self) -> impl Iterator<Item = Q::ReadOnlyItem<'b>> {
        self.cached_indices
            .iter()
            .enumerate()
            .filter_map(|(arch_id, maybe_indices)| {
                let indices = maybe_indices.as_ref()?;
                let fetch = self.cached_fetches[arch_id].as_ref()?;
                Some((indices, fetch))
            })
            .flat_map(|(indices, fetch)| {
                indices
                    .iter()
                    .map(|&index| unsafe { Q::fetch_read_only(fetch, index) })
            })
    }

    pub fn par_iter<'b>(&'b self) -> impl ParallelIterator<Item = Q::ReadOnlyItem<'b>> {
        self.cached_indices
            .par_iter()
            .enumerate()
            .filter_map(|(arch_id, maybe_indices)| {
                let indices = maybe_indices.as_ref()?;
                let fetch = self.cached_fetches[arch_id].as_ref()?;
                Some((indices, fetch))
            })
            .flat_map(|(indices, fetch)| {
                indices
                    .par_iter()
                    .map(|&index| unsafe { Q::fetch_read_only(fetch, index) })
            })
    }

    pub fn iter_mut<'b>(&'b mut self) -> impl Iterator<Item = Q::Item<'b>> {
        let fetches = &self.cached_fetches;
        self.cached_indices
            .iter()
            .enumerate()
            .filter_map(|(arch_id, maybe_indices)| {
                let indices = maybe_indices.as_ref()?;
                let fetch = fetches[arch_id].as_ref()?;
                Some((indices, fetch))
            })
            .flat_map(|(indices, fetch)| {
                indices
                    .iter()
                    .map(|&index| unsafe { Q::fetch_mut(fetch, index) })
            })
    }

    pub fn par_iter_mut<'b>(&'b mut self) -> impl ParallelIterator<Item = Q::Item<'b>> {
        let fetches = &self.cached_fetches;
        self.cached_indices
            .par_iter()
            .enumerate()
            .filter_map(|(arch_id, maybe_indices)| {
                let indices = maybe_indices.as_ref()?;
                let fetch = fetches[arch_id].as_ref()?;
                Some((indices, fetch))
            })
            .flat_map(|(indices, fetch)| {
                indices
                    .par_iter()
                    .map(|&index| unsafe { Q::fetch_mut(fetch, index) })
            })
    }

    pub fn par_chunks<'b>(
        &'b self,
        chunk_size: usize,
    ) -> impl ParallelIterator<Item = QueryReadOnlyChunk<'b, Q>> {
        self.cached_indices
            .par_iter()
            .enumerate()
            .filter_map(|(arch_id, maybe_indices)| {
                let indices = maybe_indices.as_ref()?;
                let fetch = self.cached_fetches[arch_id].as_ref()?;
                Some((indices, fetch))
            })
            .flat_map(move |(indices, fetch)| {
                indices
                    .par_chunks(chunk_size)
                    .map(|chunk| QueryReadOnlyChunk {
                        indices: chunk,
                        fetch,
                    })
            })
    }

    pub fn par_chunks_mut<'b>(
        &'b mut self,
        chunk_size: usize,
    ) -> impl ParallelIterator<Item = QueryMutableChunk<'b, Q>> {
        let fetches = &self.cached_fetches;
        self.cached_indices
            .par_iter()
            .enumerate()
            .filter_map(|(arch_id, maybe_indices)| {
                let indices = maybe_indices.as_ref()?;
                let fetch = fetches[arch_id].as_ref()?;
                Some((indices, fetch))
            })
            .flat_map(move |(indices, fetch)| {
                indices
                    .par_chunks(chunk_size)
                    .map(|chunk| QueryMutableChunk {
                        indices: chunk,
                        fetch,
                    })
            })
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    pub fn get<'w>(&'w self, entity: &Entity) -> Option<Q::ReadOnlyItem<'w>> {
        unsafe {
            let cell_ptr = REGISTRY.get_ptr(entity.registry_index as usize);
            let arch_id = (*cell_ptr).archetype_id;
            let row_idx = (*cell_ptr).idx;

            let fetch = self.cached_fetches.get(arch_id.id() as usize)?.as_ref()?;
            Some(Q::fetch_read_only(fetch, row_idx as usize))
        }
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    pub fn get_mut<'w>(&'w mut self, entity: &Entity) -> Option<Q::Item<'w>> {
        unsafe {
            let cell_ptr = REGISTRY.get_ptr(entity.registry_index as usize);
            let arch_id = (*cell_ptr).archetype_id;
            let row_idx = (*cell_ptr).idx;

            let fetch = self.cached_fetches.get(arch_id.id() as usize)?.as_ref()?;
            Some(Q::fetch_mut(fetch, row_idx as usize))
        }
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    /// # Safety
    /// The caller must ensure that the entity handle belongs to a matching archetype of the query.
    pub unsafe fn get_unchecked<'w>(&'w self, entity: &Entity) -> Q::ReadOnlyItem<'w> {
        unsafe {
            let cell_ptr = REGISTRY.get_ptr(entity.registry_index as usize);
            let arch_id = (*cell_ptr).archetype_id;
            let row_idx = (*cell_ptr).idx;

            let fetch = self
                .cached_fetches
                .get_unchecked(arch_id.id() as usize)
                .as_ref()
                .unwrap_unchecked();
            Q::fetch_read_only(fetch, row_idx as usize)
        }
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    /// # Safety
    /// The caller must ensure that the entity handle belongs to a matching archetype of the query.
    pub unsafe fn get_mut_unchecked<'w>(&'w mut self, entity: &Entity) -> Q::Item<'w> {
        unsafe {
            let cell_ptr = REGISTRY.get_ptr(entity.registry_index as usize);

            let arch_id = (*cell_ptr).archetype_id;
            let row_idx = (*cell_ptr).idx;

            let fetch = self.cached_fetches[arch_id.id() as usize]
                .as_ref()
                .unwrap_unchecked();
            Q::fetch_mut(fetch, row_idx as usize)
        }
    }

    /// Returns a raw slice of the cached fetch states for each archetype.
    ///
    /// This provides direct access to the query's interior state tracking array. It allows
    /// low-level frameworks or custom batch runners to bypass standard iteration loops.
    ///
    /// # Safety
    /// Calling this method is safe, but interacting with the slice contents is highly unsafe.
    /// The caller must guarantee the following invariants:
    ///
    /// * **Aliasing Rules:** You must never invoke [`Self::fetch_mut`] concurrently on the same
    ///   row index, or across overlapping rows from multiple threads.
    /// * **Structural Modification:** The cached fetch tokens are only valid for the current frame.
    ///   Any structural change (spawning, despawning, component insertions/removals) instantly
    ///   invalidates these tokens.
    /// * **Initialization Check:** You must explicitly check if the target `Option` is `Some`
    ///   before unwrapping. Archetypes that do not match this query's filters will be `None`.
    pub unsafe fn get_fetches(&self) -> &[Option<Q::Fetch>] {
        &self.cached_fetches
    }

    /// Returns a raw slice of the cached entity row indices vectors for each archetype.
    ///
    /// Provides raw access to the internal index maps that translate a query filter match
    /// to the physical data offset inside the archetype tables.
    ///
    /// # Safety
    /// Bypasses standard query boundaries entirely. The caller must guarantee:
    ///
    /// * **Concurrency Control:** You must never read these vectors while structural mutations
    ///   are flushing. A structural flush shifts column elements via swap-remove operations,
    ///   which will make the indices in these vectors point to out-of-bounds or incorrect memory.
    pub unsafe fn get_indices(&self) -> &[Option<Vec<usize>>] {
        &self.cached_indices
    }

    /// Unsafely extracts an immutable item view from the provided fetch token at the specified dense row index.
    ///
    /// # Safety
    /// * The `fetch` state must be valid, initialized (`Some`), and sourced directly from [`Self::get_fetches`].
    /// * The `index` must be strictly less than the length of the target archetype's data rows.
    /// * No mutable references ([`Self::fetch_mut`]) can be actively held for this row index on any thread.
    #[inline(always)]
    pub unsafe fn fetch_read_only(&self, fetch: &Q::Fetch, index: usize) -> Q::ReadOnlyItem<'q> {
        unsafe { Q::fetch_read_only(fetch, index) }
    }

    /// Unsafely extracts a mutable item view from the provided fetch token at the specified dense row index.
    ///
    /// # Safety
    /// * The `fetch` state must be valid, initialized (`Some`), and sourced directly from [`Self::get_fetches`].
    /// * The `index` must be strictly less than the length of the target archetype's data rows.
    /// * **Strict Aliasing Rule:** You must guarantee absolute exclusive access to this row. No other
    ///   thread or task may hold a read-only or mutable reference to this exact element simultaneously.
    #[inline(always)]
    pub unsafe fn fetch_mut(&self, fetch: &Q::Fetch, index: usize) -> Q::Item<'q> {
        unsafe { Q::fetch_mut(fetch, index) }
    }
}

impl<'q, 'a, Q: QueryData, F: QueryFilter> SystemParam for Query<'q, 'a, Q, F> {
    fn init_access(system_meta: &mut SystemMeta) {
        let mut local_reads = AccessVec::new();
        let mut local_writes = AccessVec::new();
        let mut local_with = AccessVec::new();
        let mut local_without = AccessVec::new();

        Q::collect_access(&mut local_reads, &mut local_writes);
        F::collect_filter(&mut local_with, &mut local_without);
        let has_intra_conflict = local_writes.iter().any(|w| local_reads.contains(w));
        let mut unique_writes = FxHashSet::default();
        let has_duplicate_writes = local_writes.iter().any(|w| !unique_writes.insert(w));

        if has_intra_conflict || has_duplicate_writes {
            panic!(
                "❌ ECS INTRA-QUERY ARGUMENT CONFLICT in '{}': Query<{}, {}> contains internal overlaps!",
                system_meta.get_func_name(),
                std::any::type_name::<Q>(),
                std::any::type_name::<F>()
            );
        }
        let mut has_inter_conflict = false;

        for id in local_writes.iter() {
            if system_meta.has_component_write(id) || system_meta.has_component_read(id) {
                has_inter_conflict = true;
                break;
            }
        }
        if !has_inter_conflict {
            for id in local_reads.iter() {
                if system_meta.has_component_write(id) {
                    has_inter_conflict = true;
                    break;
                }
            }
        }

        if has_inter_conflict {
            let is_disjoint = local_without
                .iter()
                .any(|wo| system_meta.has_with_filter(wo))
                || local_with.iter().any(|w| system_meta.has_without_filter(w));

            if !is_disjoint {
                panic!(
                    "❌ ECS SYSTEM ARGUMENT CONFLICT in '{}': Overlapping queries break aliasing rules without disjoint QueryFilters!\n\
                    -> Error triggered by parameter: Query<{}, {}>",
                    system_meta.get_func_name(),
                    std::any::type_name::<Q>(),
                    std::any::type_name::<F>()
                );
            }
        }

        for id in local_reads.iter() {
            system_meta.add_component_read(*id);
        }
        for id in local_writes.iter() {
            system_meta.add_component_write(*id);
        }
        for id in local_with.iter() {
            system_meta.add_with_filter(*id);
        }
        for id in local_without.iter() {
            system_meta.add_without_filter(*id);
        }
    }

    fn get_param(world: &World) -> Self {
        let query = Query::<Q, F>::new(&world.archetypes_manager.archetypes);

        unsafe { std::mem::transmute::<Query<'_, '_, Q, F>, Query<'_, '_, Q, F>>(query) }
    }
}
