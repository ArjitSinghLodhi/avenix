pub mod filter;
pub(crate) mod parallel_query;
mod query_data_params;
pub(crate) mod single;

use dashmap::DashMap;
pub use query_data_params::Has;

pub use filter::*;

use parking_lot::{Mutex, RwLockReadGuard};
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use rustc_hash::{FxBuildHasher, FxHashSet};
use std::{
    any::{TypeId, type_name},
    mem::transmute,
    sync::Arc,
};

use crate::{
    entity::{ENTITY_LOCATION_REGISTRY, Entity, RegistryData},
    system::{AccessVec, SystemMeta, SystemParam},
    world::{
        World,
        archetypes::{Archetype, ArchetypeId},
    },
};

pub trait QueryData {
    type Item<'w>: Send;
    type ReadOnlyItem<'w>: Send;
    type Fetch<'w>: Sync;

    fn matches(archetype: &Archetype) -> bool;

    /// # Safety
    ///
    /// Implementations must guarantee that the returned `Self::Fetch` value contains valid,
    /// initialized metadata, pointers, or references that remain valid for the duration of
    /// the borrow layout. It must not lead to out-of-bounds memory access or invalid type casting.
    unsafe fn init_fetch<'w>(archetype: &'w Archetype) -> Self::Fetch<'w>;
    fn collect_access(reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>);

    /// # Safety
    ///
    /// * `fetch` must be a valid state originally produced by `init_fetch` for the current archetype.
    /// * `index` must be strictly within the bounds of the allocated entity array for this archetype.
    /// * The caller must ensure that unique, mutable access to the underlying item at `index`
    ///   is maintained globally (no aliasing reads or writes).
    unsafe fn fetch_mut<'w, 'b>(fetch: &Self::Fetch<'b>, index: usize) -> Self::Item<'w>;

    /// # Safety
    ///
    /// * `fetch` must be a valid state originally produced by `init_fetch`.
    /// * `index` must be strictly within bounds.
    /// * Shared access must be synchronized; no concurrent mutable borrows may exist for this item.
    unsafe fn fetch_read_only<'w, 'b>(
        fetch: &Self::Fetch<'b>,
        index: usize,
    ) -> Self::ReadOnlyItem<'w>;
}

pub struct QueryReadOnlyChunk<'b, 'q, Q: QueryData, F: QueryFilter> {
    start_idx: usize,
    end_idx: usize,
    fetch: &'b Q::Fetch<'q>,
    filter_data: &'b F::FilterData<'q>,
}

impl<'b, 'q, Q: QueryData, F: QueryFilter> QueryReadOnlyChunk<'b, 'q, Q, F> {
    pub fn iter<'d>(&'d self) -> impl Iterator<Item = Q::ReadOnlyItem<'d>> {
        let fetch = self.fetch;
        let filter_data = self.filter_data;
        (self.start_idx..self.end_idx)
            .filter(move |&index| F::matches_row(filter_data, index))
            .map(move |index| unsafe { Q::fetch_read_only(fetch, index) })
    }
}

pub struct QueryMutableChunk<'b, 'q, Q: QueryData, F: QueryFilter> {
    start_idx: usize,
    end_idx: usize,
    fetch: &'b Q::Fetch<'q>,
    filter_data: &'b F::FilterData<'q>,
}

impl<'b, 'q, Q: QueryData, F: QueryFilter> QueryMutableChunk<'b, 'q, Q, F> {
    pub fn iter<'c>(&'c self) -> impl Iterator<Item = Q::ReadOnlyItem<'c>> {
        let fetch = self.fetch;
        let filter_data = self.filter_data;
        (self.start_idx..self.end_idx)
            .filter(move |&index| F::matches_row(filter_data, index))
            .map(move |index| unsafe { Q::fetch_read_only(fetch, index) })
    }

    pub fn iter_mut<'c>(&'c mut self) -> impl Iterator<Item = Q::Item<'c>> {
        let fetch = self.fetch;
        let filter_data = self.filter_data;
        (self.start_idx..self.end_idx)
            .filter(move |&index| F::matches_row(filter_data, index))
            .map(move |index| unsafe { Q::fetch_mut(fetch, index) })
    }
}

pub struct MatchedArchetypeData<'q, F: QueryFilter> {
    row_count: usize,
    filter_data: F::FilterData<'q>,
}

impl<'q, F: QueryFilter> MatchedArchetypeData<'q, F> {
    #[inline(always)]
    pub fn row_count(&self) -> usize {
        self.row_count
    }

    #[inline(always)]
    pub fn filter_data(&self) -> &F::FilterData<'q> {
        &self.filter_data
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuerySingleError {
    NoMatches,
    MultipleMatches,
}

pub struct Query<'q, Q: QueryData, F: QueryFilter = EmptyQueryFilter> {
    cached_fetches: Vec<Option<Q::Fetch<'q>>>,
    cached_archetypes: Vec<Option<MatchedArchetypeData<'q, F>>>,
    entity_location_registry_guard: RwLockReadGuard<'q, Vec<RegistryData>>,
}

unsafe impl<'q, Q: QueryData, F: QueryFilter> Sync for Query<'q, Q, F> {}

impl<'q, Q: QueryData, F: QueryFilter> Query<'q, Q, F> {
    pub(crate) fn new(
        archetypes_map: &'q Arc<DashMap<ArchetypeId, Mutex<Archetype>, FxBuildHasher>>,
    ) -> Self {
        let len = archetypes_map.len();

        let mut cached_fetches = (0..len)
            .map(|_| None)
            .collect::<Vec<Option<Q::Fetch<'q>>>>();

        let mut cached_archetypes = (0..len)
            .map(|_| None)
            .collect::<Vec<Option<MatchedArchetypeData<'q, F>>>>();

        for arch_mutex in archetypes_map.iter() {
            let arch = arch_mutex.lock();
            let arch_id = arch.id() as usize;

            if Q::matches(&arch) && F::matches(&arch) {
                let fetch = unsafe {
                    std::mem::transmute::<<Q as QueryData>::Fetch<'_>, <Q as QueryData>::Fetch<'_>>(
                        Q::init_fetch(&arch),
                    )
                };

                let filter_data = F::init_filter_data(&arch);
                let row_count = arch.entities.read().len();

                cached_fetches[arch_id] = Some(fetch);
                cached_archetypes[arch_id] = Some(MatchedArchetypeData {
                    row_count,
                    filter_data,
                });
            }
        }

        let entity_location_registry_guard = unsafe {
            transmute::<
                RwLockReadGuard<'_, Vec<RegistryData>>,
                RwLockReadGuard<'q, Vec<RegistryData>>,
            >(ENTITY_LOCATION_REGISTRY.read())
        };

        Self {
            cached_fetches,
            cached_archetypes,
            entity_location_registry_guard,
        }
    }

    pub fn get_single<'w>(&'w self) -> Result<Q::ReadOnlyItem<'w>, QuerySingleError> {
        let mut target_arch_id: Option<usize> = None;
        let mut target_row_idx: Option<usize> = None;
        let mut match_count: usize = 0;

        for (arch_id, maybe_arch) in self.cached_archetypes.iter().enumerate() {
            if let Some(arch) = maybe_arch {
                if arch.row_count == 0 {
                    continue;
                }

                for row_idx in 0..arch.row_count {
                    if F::matches_row(&arch.filter_data, row_idx) {
                        match_count += 1;

                        if match_count > 1 {
                            return Err(QuerySingleError::MultipleMatches);
                        }

                        target_arch_id = Some(arch_id);
                        target_row_idx = Some(row_idx);
                    }
                }
            }
        }

        let arch_id = target_arch_id.ok_or(QuerySingleError::NoMatches)?;
        let row_idx = target_row_idx.ok_or(QuerySingleError::NoMatches)?;

        let fetch_state = self.cached_fetches[arch_id]
            .as_ref()
            .ok_or(QuerySingleError::NoMatches)?;
        unsafe { Ok(Q::fetch_read_only(fetch_state, row_idx)) }
    }

    pub fn get_single_mut<'w>(&'w mut self) -> Result<Q::Item<'w>, QuerySingleError> {
        let mut target_arch_id: Option<usize> = None;
        let mut target_row_idx: Option<usize> = None;
        let mut match_count: usize = 0;

        for (arch_id, maybe_arch) in self.cached_archetypes.iter().enumerate() {
            if let Some(arch) = maybe_arch {
                if arch.row_count == 0 {
                    continue;
                }

                for row_idx in 0..arch.row_count {
                    if F::matches_row(&arch.filter_data, row_idx) {
                        match_count += 1;

                        if match_count > 1 {
                            return Err(QuerySingleError::MultipleMatches);
                        }

                        target_arch_id = Some(arch_id);
                        target_row_idx = Some(row_idx);
                    }
                }
            }
        }

        let arch_id = target_arch_id.ok_or(QuerySingleError::NoMatches)?;
        let row_idx = target_row_idx.ok_or(QuerySingleError::NoMatches)?;

        let fetch_state = self.cached_fetches[arch_id]
            .as_ref()
            .ok_or(QuerySingleError::NoMatches)?;
        unsafe { Ok(Q::fetch_mut(fetch_state, row_idx)) }
    }

    pub fn single<'w>(&'w self) -> Q::ReadOnlyItem<'w> {
        self.get_single().unwrap_or_else(|err| {
            panic!("QuerySingleError::{:?}", err);
        })
    }

    pub fn single_mut<'w>(&'w mut self) -> Q::Item<'w> {
        self.get_single_mut().unwrap_or_else(|err| {
            panic!("QuerySingleError::{:?}", err);
        })
    }

    /// Returns the total number of entity indices across all matching archetypes,
    /// fully accounting for any dynamic filters.
    pub fn filtered_entities(&self) -> usize {
        let mut count = 0;
        for arch in self.cached_archetypes.iter().flatten() {
            if arch.row_count == 0 {
                continue;
            }
            for row_idx in 0..arch.row_count {
                if F::matches_row(&arch.filter_data, row_idx) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Returns the total number of entities across all matched archetypes,
    /// without accounting for dynamic filters.
    pub fn base_entities(&self) -> usize {
        self.cached_archetypes
            .iter()
            .flatten()
            .map(|arch| arch.row_count)
            .sum()
    }

    pub fn iter<'b>(&'b self) -> impl Iterator<Item = Q::ReadOnlyItem<'b>> {
        self.cached_archetypes
            .iter()
            .enumerate()
            .filter_map(|(arch_id, maybe_arch)| {
                let arch = maybe_arch.as_ref()?;
                let fetch = self.cached_fetches[arch_id].as_ref()?;
                Some((arch.row_count, fetch, &arch.filter_data))
            })
            .flat_map(|(total_rows, fetch, filter_data)| {
                (0..total_rows)
                    .filter(move |&row_idx| F::matches_row(filter_data, row_idx))
                    .map(move |row_idx| unsafe { Q::fetch_read_only(fetch, row_idx) })
            })
    }

    pub fn iter_mut<'b>(&'b mut self) -> impl Iterator<Item = Q::Item<'b>> {
        let fetches = &self.cached_fetches;
        self.cached_archetypes
            .iter()
            .enumerate()
            .filter_map(move |(arch_id, maybe_arch)| {
                let arch = maybe_arch.as_ref()?;
                let fetch = fetches[arch_id].as_ref()?;
                Some((arch.row_count, fetch, &arch.filter_data))
            })
            .flat_map(|(total_rows, fetch, filter_data)| {
                (0..total_rows)
                    .filter(move |&row_idx| F::matches_row(filter_data, row_idx))
                    .map(move |row_idx| unsafe { Q::fetch_mut(fetch, row_idx) })
            })
    }

    pub fn par_iter<'b>(&'b self) -> impl ParallelIterator<Item = Q::ReadOnlyItem<'b>> {
        self.cached_archetypes
            .par_iter()
            .enumerate()
            .filter_map(|(arch_id, maybe_arch)| {
                let arch = maybe_arch.as_ref()?;
                let fetch = self.cached_fetches[arch_id].as_ref()?;
                Some((arch.row_count, fetch, &arch.filter_data))
            })
            .flat_map(|(total_rows, fetch, filter_data)| {
                (0..total_rows)
                    .into_par_iter()
                    .filter(move |&row_idx| F::matches_row(filter_data, row_idx))
                    .map(move |row_idx| unsafe { Q::fetch_read_only(fetch, row_idx) })
            })
    }

    pub fn par_iter_mut<'b>(&'b mut self) -> impl ParallelIterator<Item = Q::Item<'b>> {
        let fetches = &self.cached_fetches;
        self.cached_archetypes
            .par_iter()
            .enumerate()
            .filter_map(move |(arch_id, maybe_arch)| {
                let arch = maybe_arch.as_ref()?;
                let fetch = fetches[arch_id].as_ref()?;
                Some((arch.row_count, fetch, &arch.filter_data))
            })
            .flat_map(|(total_rows, fetch, filter_data)| {
                (0..total_rows)
                    .into_par_iter()
                    .filter(move |&row_idx| F::matches_row(filter_data, row_idx))
                    .map(move |row_idx| unsafe { Q::fetch_mut(fetch, row_idx) })
            })
    }

    pub fn par_chunks<'b>(
        &'b self,
        chunk_size: usize,
    ) -> impl ParallelIterator<Item = QueryReadOnlyChunk<'b, 'q, Q, F>> {
        self.cached_archetypes
            .par_iter()
            .enumerate()
            .filter_map(|(arch_id, maybe_arch)| {
                let arch = maybe_arch.as_ref()?;
                let fetch = self.cached_fetches[arch_id].as_ref()?;
                Some((arch.row_count, fetch, &arch.filter_data))
            })
            .flat_map(move |(total_rows, fetch, filter_data)| {
                (0..total_rows)
                    .into_par_iter()
                    .step_by(chunk_size)
                    .map(move |start_idx| {
                        let end_idx = std::cmp::min(start_idx + chunk_size, total_rows);
                        QueryReadOnlyChunk {
                            start_idx,
                            end_idx,
                            fetch,
                            filter_data,
                        }
                    })
            })
    }

    pub fn par_chunks_mut<'b>(
        &'b mut self,
        chunk_size: usize,
    ) -> impl ParallelIterator<Item = QueryMutableChunk<'b, 'q, Q, F>> {
        let fetches = &self.cached_fetches;
        self.cached_archetypes
            .par_iter()
            .enumerate()
            .filter_map(move |(arch_id, maybe_arch)| {
                let arch = maybe_arch.as_ref()?;
                let fetch = fetches[arch_id].as_ref()?;
                Some((arch.row_count, fetch, &arch.filter_data))
            })
            .flat_map(move |(total_rows, fetch, filter_data)| {
                (0..total_rows)
                    .into_par_iter()
                    .step_by(chunk_size)
                    .map(move |start_idx| {
                        let end_idx = std::cmp::min(start_idx + chunk_size, total_rows);
                        QueryMutableChunk {
                            start_idx,
                            end_idx,
                            fetch,
                            filter_data,
                        }
                    })
            })
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    pub fn get<'w>(&'w self, entity: &Entity) -> Option<Q::ReadOnlyItem<'w>> {
        unsafe {
            let (arch_id, row_index) = self.get_row_and_arch(entity);

            let fetch = self.cached_fetches.get(arch_id.id() as usize)?.as_ref()?;
            Some(Q::fetch_read_only(fetch, row_index as usize))
        }
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    pub fn get_mut<'w>(&'w mut self, entity: &Entity) -> Option<Q::Item<'w>> {
        unsafe {
            let (arch_id, row_index) = self.get_row_and_arch(entity);

            let fetch = self.cached_fetches.get(arch_id.id() as usize)?.as_ref()?;
            Some(Q::fetch_mut(fetch, row_index as usize))
        }
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    /// # Safety
    /// The caller must ensure that the entity handle belongs to a matching archetype of the query.
    pub unsafe fn get_unchecked<'w>(&'w self, entity: &Entity) -> Q::ReadOnlyItem<'w> {
        unsafe {
            let (arch_id, row_index) = self.get_row_and_arch(entity);
            let fetch = self
                .cached_fetches
                .get_unchecked(arch_id.id() as usize)
                .as_ref()
                .unwrap_unchecked();
            Q::fetch_read_only(fetch, row_index as usize)
        }
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    /// # Safety
    /// The caller must ensure that the entity handle belongs to a matching archetype of the query.
    pub unsafe fn get_mut_unchecked<'w>(&'w mut self, entity: &Entity) -> Q::Item<'w> {
        unsafe {
            let (arch_id, row_index) = self.get_row_and_arch(entity);
            let fetch = self.cached_fetches[arch_id.id() as usize]
                .as_ref()
                .unwrap_unchecked();
            Q::fetch_mut(fetch, row_index as usize)
        }
    }

    fn get_row_and_arch(&self, entity: &Entity) -> (ArchetypeId, u32) {
        unsafe {
            let cell_ptr = self
                .entity_location_registry_guard
                .as_ptr()
                .add(entity.registry_index() as usize);
            let arch_id = (*cell_ptr).archetype_id();
            let row_index = (*cell_ptr).index();
            (arch_id, row_index)
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
    pub unsafe fn get_fetches(&self) -> &[Option<Q::Fetch<'q>>] {
        &self.cached_fetches
    }

    /// Returns a raw slice of the cached matched archetype metadata records.
    ///
    /// Provides low-level frameworks and custom batch runners with direct access to the
    /// structural row bounds and dynamic filter states unified inside a single array.
    ///
    /// # Safety
    /// Bypasses standard query safety boundaries entirely. The caller must guarantee:
    ///
    /// * **State Lifetime Invalidation:** The underlying filter records inside the matching configurations
    ///   are transient structures valid only for the immediate frame context. You must never hold,
    ///   cache, or propagate these references beyond the boundary of the current execution frame.
    #[inline(always)]
    pub unsafe fn get_matched_archetypes_metadata(&self) -> &[Option<MatchedArchetypeData<'q, F>>] {
        &self.cached_archetypes
    }

    /// Unsafely evaluates whether a specific row index within an archetype matches the query's filters.
    ///
    /// # Safety
    /// * The `filter_data` reference must be valid, initialized, and sourced directly from a matched element inside [`Self::get_matched_archetypes_metadata`].
    /// * The `index` must be strictly less than the corresponding `row_count` exposed by that archetype's record.
    #[inline(always)]
    pub unsafe fn matches_row(&self, filter_data: &F::FilterData<'q>, index: usize) -> bool {
        F::matches_row(filter_data, index)
    }

    /// Unsafely extracts an immutable item view from the provided fetch token at the specified dense row index.
    ///
    /// # Safety
    /// * The `fetch` state must be valid, initialized (`Some`), and sourced directly from [`Self::get_fetches`].
    /// * The `index` must be strictly less than the length of the target archetype's data rows.
    /// * No mutable references ([`Self::fetch_mut`]) can be actively held for this row index on any thread.
    #[inline(always)]
    pub unsafe fn fetch_read_only(
        &self,
        fetch: &Q::Fetch<'q>,
        index: usize,
    ) -> Q::ReadOnlyItem<'_> {
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
    pub unsafe fn fetch_mut(&self, fetch: &Q::Fetch<'q>, index: usize) -> Q::Item<'_> {
        unsafe { Q::fetch_mut(fetch, index) }
    }
}

impl<'q, Q: QueryData, F: QueryFilter> SystemParam for Query<'q, Q, F> {
    type Item<'world> = Query<'world, Q, F>;

    fn init_access(system_meta: &mut SystemMeta) {
        let mut local_reads = AccessVec::new();
        let mut local_writes = AccessVec::new();
        let mut local_with = AccessVec::new();
        let mut local_without = AccessVec::new();

        Q::collect_access(&mut local_reads, &mut local_writes);
        F::collect_filter_access(&mut local_reads, &mut local_writes);
        F::collect_filter(&mut local_with, &mut local_without);

        let has_intra_conflict = local_writes.iter().any(|w| local_reads.contains(w));
        let mut unique_writes = FxHashSet::default();
        let has_duplicate_writes = local_writes.iter().any(|w| !unique_writes.insert(w));

        if has_intra_conflict || has_duplicate_writes {
            panic!(
                "❌ ECS INTRA-QUERY ARGUMENT CONFLICT in '{}': {} contains internal overlaps!",
                system_meta.get_func_name(),
                type_name::<Self>()
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
                -> Error triggered by parameter: {}",
                    system_meta.get_func_name(),
                    type_name::<Self>()
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

    fn get_param<'world>(world: &'world World) -> Option<Self::Item<'world>> {
        let query = Query::<Q, F>::new(&world.archetypes_manager.archetypes);
        Some(query)
    }
}
