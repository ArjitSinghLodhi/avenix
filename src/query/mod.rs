pub mod filter;
mod params;

pub use params::Has;

pub use filter::*;

use fxhash::FxBuildHasher;
use indexmap::IndexSet;
use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator,
};
use std::{any::TypeId, marker::PhantomData};

use crate::{
    entity::Entity,
    extensions::{AccessVec, SystemMeta, SystemParam},
    registry::REGISTRY,
    system::AccessHashSet,
    world::{
        archetypes::{Archetype, ArchetypeId},
        storage::World,
    },
};

pub trait QueryData {
    type Item<'w>;
    type ReadOnlyItem<'w>;
    type Fetch: Send + Sync + 'static;

    fn matches(types: &IndexSet<TypeId, FxBuildHasher>) -> bool;

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

pub struct Mutable;
pub struct ReadOnly;

pub struct QuerySubChunk<'w, Q: QueryData, I> {
    safe_fetch: &'w Q::Fetch,
    sub_indices: &'w [usize],
    _marker: std::marker::PhantomData<I>,
}

impl<'w, Q: QueryData, T> QuerySubChunk<'w, Q, T> {
    #[inline(always)]
    pub fn iter<'b>(&'b self) -> impl Iterator<Item = Q::ReadOnlyItem<'b>> {
        let safe_fetch = self.safe_fetch;
        self.sub_indices
            .iter()
            .map(move |&idx| unsafe { Q::fetch_read_only(safe_fetch, idx)})
    }

    pub fn len(&self) -> usize {
        self.sub_indices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<'w, Q: QueryData> QuerySubChunk<'w, Q, Mutable> {
    #[inline(always)]
    pub fn iter_mut<'b>(&'b mut self) -> impl Iterator<Item = Q::Item<'b>> {
        let safe_fetch = self.safe_fetch;
        self.sub_indices
            .iter()
            .map(move |&idx| unsafe { Q::fetch_mut(safe_fetch, idx) })
    }
}

struct ThreadSafeFetch<'a, Q: QueryData>(&'a Q::Fetch);

unsafe impl<'a, Q: QueryData> Send for ThreadSafeFetch<'a, Q> {}
unsafe impl<'a, Q: QueryData> Sync for ThreadSafeFetch<'a, Q> {}

impl<'a, Q: QueryData> ThreadSafeFetch<'a, Q> {
    #[inline(always)]
    unsafe fn fetch_read_only<'w>(&self, index: usize) -> Q::ReadOnlyItem<'w> {
        unsafe { Q::fetch_read_only(&self.0, index) }
    }
    #[inline(always)]
    unsafe fn fetch_mut<'w>(&self, index: usize) -> Q::Item<'w> {
        unsafe { Q::fetch_mut(&self.0, index) }
    }
}

pub struct QueryArchetypeView<'a, Q: QueryData, I> {
    indices: &'a [usize],
    fetch: &'a Q::Fetch,
    total_entity_count: usize,
    archetype_id: ArchetypeId,
    _marker: std::marker::PhantomData<I>,
}

impl<'w, Q: QueryData, T> QueryArchetypeView<'w, Q, T> {
    /// Gets the length of the filtered indices slice.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    /// Gets the total length of the entities inside the archetype, including unfiltered ones.
    #[inline(always)]
    pub fn entities_len(&self) -> usize {
        self.total_entity_count
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The underlying archetype data referenced by this view remains structurally stable
    ///   (i.e., no entity movement, destruction, or layout changes occur) while the returned `Fetch` is in active use.
    /// - Memory safety is guaranteed when accessing any index up to `entities_len()`. However, to preserve
    ///   query filtering invariants, lookups should be strictly confined to the indices provided by `get_indices()`.
    /// - Aliasing rules are strictly observed: if a Fetch requests mutable access, no other references to this
    ///   archetype's component data may exist simultaneously.
    #[inline(always)]
    pub unsafe fn get_fetch(&mut self) -> &Q::Fetch {
        &self.fetch
    }

    /// # Safety
    ///
    /// - `index` must be strictly less than `entities_len()` to prevent out-of-bounds memory corruption.
    /// - The caller must guarantee that exclusive, mutable access to the underlying item at `index`
    ///   is maintained globally (no simultaneous aliasing reads or writes from other queries/threads).
    #[inline(always)]
    pub unsafe fn fetch_mut<'a>(&'a self, index: usize) -> Q::Item<'a> {
        // Fix: Changed return lifetime from 'w to 'a to tie the returned component borrow
        // to the short-lived accessor scope, preventing dangerous concurrent mutable aliasing.
        unsafe { Q::fetch_mut(&self.fetch, index) }
    }

    /// # Safety
    ///
    /// - `index` must be strictly less than `entities_len()`.
    /// - Shared read access must be synchronized; no concurrent mutable borrows (`&mut`) may exist
    ///   for this specific entity item anywhere in the execution frame.
    #[inline(always)]
    pub unsafe fn fetch_read_only<'a>(&'a self, index: usize) -> Q::ReadOnlyItem<'a> {
        // Fix: Bound the read-only item to the short accessor lifetime 'a
        unsafe { Q::fetch_read_only(&self.fetch, index) }
    }

    #[inline(always)]
    pub fn get_indices(&self) -> &'w [usize] {
        self.indices
    }
}

impl<'a, Q: QueryData, T> QueryArchetypeView<'a, Q, T> {
    pub fn iter<'b>(&'b self) -> impl Iterator<Item = Q::ReadOnlyItem<'b>> {
        self.indices
            .iter()
            .map(move |idx| unsafe { Q::fetch_read_only(&self.fetch, *idx) })
    }

    pub fn par_iter<'b>(&'b self) -> impl IndexedParallelIterator<Item = Q::ReadOnlyItem<'b>>
    where
        Q::ReadOnlyItem<'b>: Send,
    {
        let safe_fetch = ThreadSafeFetch::<Q>(self.fetch);

        self.indices
            .into_par_iter()
            .map(move |i| unsafe { safe_fetch.fetch_read_only(*i) })
    }

    pub fn par_chunks<'b>(
        &'b self,
        chunk_size: usize,
    ) -> impl IndexedParallelIterator<Item = QuerySubChunk<'b, Q, ReadOnly>>
    where
        Q::ReadOnlyItem<'b>: Send,
    {
        assert!(chunk_size > 0, "Chunk size must be greater than zero");
        let indices_slice = self.indices;
        let len = self.indices.len();
        let safe_fetch = self.fetch;

        (0..len)
            .into_par_iter()
            .step_by(chunk_size)
            .map(move |start_pos| {
                let end_pos = std::cmp::min(start_pos + chunk_size, len);
                QuerySubChunk {
                    safe_fetch: safe_fetch,
                    sub_indices: &indices_slice[start_pos..end_pos],
                    _marker: std::marker::PhantomData,
                }
            })
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    pub fn get(&self, entity: &Entity) -> Option<Q::ReadOnlyItem<'a>> {
        unsafe {
            let cell = REGISTRY.get_ptr(entity.registry_index as usize);
            let arch_id = (*cell).archetype_id.id();
            if arch_id == self.archetype_id.id() {
                let row_idx = (*cell).idx;
                Some(Q::fetch_read_only(&self.fetch, row_idx as usize))
            } else {
                None
            }
        }
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    /// # Safety
    /// The caller must ensure that the entity handle belongs to the archetype of this view
    pub unsafe fn get_unchecked(&self, entity: &Entity) -> Q::ReadOnlyItem<'a> {
        unsafe {
            let cell = REGISTRY.get_ptr(entity.registry_index as usize);
            let row_idx = (*cell).idx;
            Q::fetch_read_only(&self.fetch, row_idx as usize)
        }
    }
}

impl<'a, Q: QueryData> QueryArchetypeView<'a, Q, Mutable> {
    pub fn iter_mut<'b>(&'b mut self) -> impl Iterator<Item = Q::Item<'b>> {
        self.indices
            .iter()
            .map(move |idx| unsafe { Q::fetch_mut(&self.fetch, *idx) })
    }

    pub fn par_iter_mut<'b>(&'b mut self) -> impl IndexedParallelIterator<Item = Q::Item<'b>>
    where
        Q::Item<'b>: Send,
    {
        let safe_fetch = ThreadSafeFetch::<Q>(self.fetch);
        self.indices
            .into_par_iter()
            .map(move |i| unsafe { safe_fetch.fetch_mut(*i) })
    }

    pub fn par_chunks_mut<'b>(
        &'b mut self,
        chunk_size: usize,
    ) -> impl IndexedParallelIterator<Item = QuerySubChunk<'b, Q, Mutable>>
    where
        Q::Item<'b>: Send,
    {
        assert!(chunk_size > 0, "Chunk size must be greater than zero");

        let indices_slice = self.indices;
        let len = indices_slice.len();
        let safe_fetch = self.fetch;

        (0..len)
            .into_par_iter()
            .step_by(chunk_size)
            .map(move |start_pos| {
                let end_pos = std::cmp::min(start_pos + chunk_size, len);
                QuerySubChunk {
                    safe_fetch: safe_fetch,
                    sub_indices: &indices_slice[start_pos..end_pos],
                    _marker: std::marker::PhantomData,
                }
            })
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    pub fn get_mut(&mut self, entity: &Entity) -> Option<Q::Item<'a>> {
        unsafe {
            let cell = REGISTRY.get_ptr(entity.registry_index as usize);
            let arch_id = (*cell).archetype_id.id();
            if arch_id == self.archetype_id.id() {
                let row_idx = (*cell).idx;
                Some(Q::fetch_mut(&self.fetch, row_idx as usize))
            } else {
                None
            }
        }
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    /// # Safety
    /// The caller must ensure that the entity handle belongs to the archetype of this view
    pub unsafe fn get_mut_unchecked(&self, entity: &Entity) -> Q::Item<'a> {
        unsafe {
            let cell = REGISTRY.get_ptr(entity.registry_index as usize);
            let row_idx = (*cell).idx;
            Q::fetch_mut(&self.fetch, row_idx as usize)
        }
    }
}

unsafe impl<'a, Q: QueryData, T> Send for QueryArchetypeView<'a, Q, T> {}
unsafe impl<'a, Q: QueryData, T> Sync for QueryArchetypeView<'a, Q, T> {}

#[doc(hidden)]
pub struct ThreadSafe<T>{
    pub(crate) value: T
}
unsafe impl<T> Send for ThreadSafe<T> {}
unsafe impl<T> Sync for ThreadSafe<T> {}

pub struct Query<'q, Q: QueryData, F: QueryFilter = EmptyQueryFilter> {
    matching_archetypes: Vec<Option<ThreadSafe<*const Archetype>>>,
    cached_fetches: Vec<Option<ThreadSafe<Q::Fetch>>>,
    cached_indices: Vec<Vec<usize>>,
    _marker: std::marker::PhantomData<(&'q (), F)>,
}
unsafe impl<'q, Q: QueryData, F: QueryFilter> Send for Query<'q, Q, F> {}
unsafe impl<'q, Q: QueryData, F: QueryFilter> Sync for Query<'q, Q, F> {}

impl<'q, Q: QueryData, F: QueryFilter> Query<'q, Q, F> {
    pub(crate) fn new(world: &mut World) -> Self {
        let mut matching_archetypes = (0..world.archetypes_manager.archetypes.len()).map(|_| None).collect::<Vec<Option<ThreadSafe<*const Archetype>>>>();
        let mut cached_fetches = (0..world.archetypes_manager.archetypes.len()).map(|_| None).collect::<Vec<Option<ThreadSafe<Q::Fetch>>>>();
        let mut cached_indices = vec![Vec::new(); world.archetypes_manager.archetypes.len()];

        for arch in world.archetypes_manager.archetypes.values() {
            let arch_id = arch.id();
            if Q::matches(&arch.types)
                && F::matches(&AccessHashSet {
                    set: arch.types.clone(),
                })
            {
                matching_archetypes[arch_id as usize] = Some(ThreadSafe{ value: arch as *const Archetype});
                let fetch = unsafe { Q::init_fetch(arch) };
                cached_fetches[arch_id as usize] = Some(ThreadSafe{value:fetch});
                let mut indices = (0..arch.entities.len()).collect::<Vec<usize>>();
                F::filter_indices(arch, &mut indices);
                cached_indices[arch_id as usize] = indices;
            }
        }

        Self {
            matching_archetypes,
            cached_fetches,
            cached_indices,
            _marker: PhantomData,
        }
    }

    pub fn single<'w>(&'w self) -> Option<Q::ReadOnlyItem<'w>> {
        let mut found_item = None;

        for (arch_id, indices) in self.cached_indices.iter().enumerate() {
            if indices.is_empty() {
                continue;
            }

            if found_item.is_some() || indices.len() > 1 {
                return None;
            }

            let entity_index = indices[0];

            if let Some(ThreadSafe { value:fetch_state}) = &self.cached_fetches[arch_id] {
                unsafe {
                    found_item = Some(Q::fetch_read_only(&fetch_state, entity_index));
                }
            }
        }

        found_item
    }

    pub fn single_mut<'w>(&'w mut self) -> Option<Q::Item<'w>> {
        let mut target_match = None;

        for (arch_id, indices) in self.cached_indices.iter().enumerate() {
            if indices.is_empty() {
                continue;
            }

            if target_match.is_some() || indices.len() > 1 {
                return None;
            }

            target_match = Some((arch_id, indices[0]));
        }

        if let Some((arch_id, entity_index)) = target_match
            && let Some(ThreadSafe{value: fetch_state}) = &self.cached_fetches[arch_id]
        {
            unsafe {
                return Some(Q::fetch_mut(&fetch_state, entity_index));
            }
        }

        None
    }

    pub fn matching_archetype_count(&self) -> usize {
        self.matching_archetypes.iter().flatten().count()
    }

    pub fn total_entities(&self) -> usize {
        self.matching_archetypes
            .iter()
            .flatten()
            .map(|arch| unsafe { (*arch.value).entities.len() })
            .sum()
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = QueryArchetypeView<'a, Q, ReadOnly>> {
        self.matching_archetypes
            .iter()
            .flatten()
            .map(move |arch_ptr| unsafe {
                let arch = &*arch_ptr.value;
                let arch_idx = arch.id() as usize;
                let fetch_opt = &self.cached_fetches[arch_idx];
                let fetch = &fetch_opt.as_ref().unwrap().value;
                QueryArchetypeView {
                    indices: &self.cached_indices[arch_idx],
                    fetch: fetch,
                    total_entity_count: arch.entities.len(),
                    archetype_id: arch.id,
                    _marker: PhantomData,
                }
            })
    }

    pub fn par_iter<'a>(
        &'a self,
    ) -> impl ParallelIterator<Item = QueryArchetypeView<'a, Q, ReadOnly>> {
        self.matching_archetypes
            .par_iter()
            .flatten()
            .map(|arch_ptr| {
                let arch = unsafe { &*arch_ptr.value };
                let arch_idx = arch.id() as usize;
                let fetch_opt = &self.cached_fetches[arch_idx];
                let fetch = &fetch_opt.as_ref().unwrap().value;
                QueryArchetypeView {
                    indices: &self.cached_indices[arch_idx],
                    fetch: fetch,
                    total_entity_count: arch.entities.len(),
                    archetype_id: arch.id,
                    _marker: PhantomData,
                }
            })
    }

    pub fn iter_mut<'a>(&'a mut self) -> impl Iterator<Item = QueryArchetypeView<'a, Q, Mutable>> {
        self.matching_archetypes
            .iter()
            .flatten()
            .map(|arch_ptr| unsafe {
                let arch = &*arch_ptr.value;
                let arch_idx = arch.id() as usize;
                let fetch_opt = &self.cached_fetches[arch_idx];
                let fetch = &fetch_opt.as_ref().unwrap().value;
                QueryArchetypeView {
                    indices: &self.cached_indices[arch_idx],
                    fetch,
                    total_entity_count: arch.entities.len(),
                    archetype_id: arch.id,
                    _marker: PhantomData,
                }
            })
    }

    pub fn par_iter_mut<'a>(
        &'a mut self,
    ) -> impl ParallelIterator<Item = QueryArchetypeView<'a, Q, Mutable>> {
        self.matching_archetypes
            .par_iter()
            .flatten()
            .map(|arch_ptr| {
                let arch = unsafe { &*arch_ptr.value };
                let arch_idx = arch.id() as usize;
                let fetch_opt = &self.cached_fetches[arch_idx];
                let fetch = &fetch_opt.as_ref().unwrap().value;
                QueryArchetypeView {
                    indices: &self.cached_indices[arch_idx],
                    fetch,
                    total_entity_count: arch.entities.len(),
                    archetype_id: arch.id,
                    _marker: PhantomData,
                }
            })
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    pub fn get(&self, entity: &Entity) -> Option<Q::ReadOnlyItem<'q>> {
        unsafe {
            let cell_ptr = REGISTRY.get_ptr(entity.registry_index as usize);

            let arch_id = (*cell_ptr).archetype_id;
            let row_idx = (*cell_ptr).idx;

            let fetch = &self.cached_fetches[arch_id.id() as usize].as_ref()?.value;
            Some(Q::fetch_read_only(fetch, row_idx as usize))
        }
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    pub fn get_mut(&mut self, entity: &Entity) -> Option<Q::Item<'q>> {
        unsafe {
            let cell_ptr = REGISTRY.get_ptr(entity.registry_index as usize);

            let arch_id = (*cell_ptr).archetype_id;
            let row_idx = (*cell_ptr).idx;

            let fetch = &self.cached_fetches[arch_id.id() as usize].as_ref()?.value;
            Some(Q::fetch_mut(fetch, row_idx as usize))
        }
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    /// # Safety
    /// The caller must ensure that the entity handle belongs to a matching archetype of the query.
    pub unsafe fn get_unchecked(&self, entity: &Entity) -> Q::ReadOnlyItem<'q> {
        unsafe {
            let cell_ptr = REGISTRY.get_ptr(entity.registry_index as usize);

            let arch_id = (*cell_ptr).archetype_id;
            let row_idx = (*cell_ptr).idx;

            let fetch = &self.cached_fetches[arch_id.id() as usize].as_ref()
                .unwrap_unchecked()
                .value;
            Q::fetch_read_only(fetch, row_idx as usize)
        }
    }

    /// Random access lookup via Entity handle.
    ///
    /// Note: it does not respect dynamic filters like `Changed<T>` or `Added<T>`
    /// # Safety
    /// The caller must ensure that the entity handle belongs to a matching archetype of the query.
    pub unsafe fn get_mut_unchecked(&mut self, entity: &Entity) -> Q::Item<'q> {
        unsafe {
            let cell_ptr = REGISTRY.get_ptr(entity.registry_index as usize);

            let arch_id = (*cell_ptr).archetype_id;
            let row_idx = (*cell_ptr).idx;

            let fetch = &self.cached_fetches[arch_id.id() as usize].as_ref()
                .unwrap_unchecked()
                .value;
            Q::fetch_mut(fetch, row_idx as usize)
        }
    }
}

impl<'q, Q: QueryData + 'static, F: QueryFilter + 'static> SystemParam for Query<'q, Q, F> {
    fn init_access(system_meta: &mut SystemMeta) {
        let mut local_reads = AccessVec::new();
        let mut local_writes = AccessVec::new();
        let mut local_with = AccessVec::new();
        let mut local_without = AccessVec::new();

        Q::collect_access(&mut local_reads, &mut local_writes);
        F::collect_filter(&mut local_with, &mut local_without);
        let has_intra_conflict = local_writes.iter().any(|w| local_reads.contains(w));
        let mut unique_writes = std::collections::HashSet::new();
        let has_duplicate_writes = local_writes.iter().any(|w| !unique_writes.insert(w));

        if has_intra_conflict || has_duplicate_writes {
            panic!(
                "❌ ECS INTRA-QUERY ARGUMENT CONFLICT in '{}': Query<{}, {}> contains internal overlaps!",
                system_meta.name,
                std::any::type_name::<Q>(),
                std::any::type_name::<F>()
            );
        }
        let mut has_inter_conflict = false;

        for id in local_writes.iter() {
            if system_meta.component_writes.contains(id) || system_meta.component_reads.contains(id)
            {
                has_inter_conflict = true;
                break;
            }
        }
        if !has_inter_conflict {
            for id in local_reads.iter() {
                if system_meta.component_writes.contains(id) {
                    has_inter_conflict = true;
                    break;
                }
            }
        }

        if has_inter_conflict {
            let is_disjoint = local_without
                .iter()
                .any(|wo| system_meta.with_filters.contains(wo))
                || local_with
                    .iter()
                    .any(|w| system_meta.without_filters.contains(w));

            if !is_disjoint {
                panic!(
                    "❌ ECS SYSTEM ARGUMENT CONFLICT in '{}': Overlapping queries break aliasing rules without disjoint QueryFilters!\n\
                    -> Error triggered by parameter: Query<{}, {}>",
                    system_meta.name,
                    std::any::type_name::<Q>(),
                    std::any::type_name::<F>()
                );
            }
        }

        for id in local_reads.iter() {
            system_meta.component_reads.insert(*id);
        }
        for id in local_writes.iter() {
            system_meta.component_writes.insert(*id);
        }
        for id in local_with.iter() {
            system_meta.with_filters.insert(*id);
        }
        for id in local_without.iter() {
            system_meta.without_filters.insert(*id);
        }
    }

    fn get_param(world: &mut World) -> Self {
        Query::<Q, F>::new(world)
    }
}
