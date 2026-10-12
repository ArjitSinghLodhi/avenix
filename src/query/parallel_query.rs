use std::{marker::PhantomData, sync::Arc};

use dashmap::DashMap;
use parking_lot::Mutex;
use rustc_hash::FxBuildHasher;

use crate::{
    app_impl::ConfigurationContext,
    extensions::{Archetype, World},
    query::{EmptyQueryFilter, Query, QueryData, QueryFilter},
    system::{AccessVec, SystemMeta, SystemParam},
    world::archetypes::ArchetypeId,
};

/// A thread-safe, clonable handle providing detached remote access to the engine's archetype data blocks.
///
/// `ParallelQueryAccessor` can be passed to external or background worker threads, allowing them to
/// concurrently read or modify component data arrays outside the main execution path. It can be obtained
/// directly from the world layer via [`.get_par_query_accessor()`] before building the app.
///
/// # Initialization Requirement
/// Unlike other parallel handles, this handle must be initialized prior to app compilation. This ensures the
/// engine can automatically register and configure the underlying demand-driven component tracking.
///
/// # Thread Safety
/// Standard concurrent reads and writes for component columns are completely safe without any extra rules.
/// For specific rules regarding cloning `Entity` handles, lookups, or using [`query.get()`] out-of-band,
/// please refer to the [`Entity`] documentation.
///
/// # Deadlock Safety Rules
/// Cross-thread parallel access is entirely safe. However, because the underlying storage utilizes granular,
/// column-level `RwLocks` to enable simultaneous multi-threaded table access, nesting parallel query scopes
/// incorrectly on the *same thread* will freeze execution:
/// * **Same-Thread Deadlocks:** Nesting scopes on a single thread will cause a freeze if standard borrowing rules are broken.
/// * **Overlapping Access:** Opening a mutable query scope while an active scope (mutable or immutable)
///   targets overlapping components on that same thread will trigger a deadlock.
///
/// [`.get_par_query_accessor()`]: crate::world::storage::World::get_par_query_accessor
/// [`Entity`]: crate::entity::Entity
/// [`query.get()`]: crate::query::Query::get
pub struct ParallelQueryAccessor<Q: QueryData, F: QueryFilter = EmptyQueryFilter> {
    pub(crate) archetypes_map: Arc<DashMap<ArchetypeId, Mutex<Archetype>, FxBuildHasher>>,
    pub(crate) _marker: PhantomData<(Q, F)>,
}

impl<Q: QueryData, F: QueryFilter> Clone for ParallelQueryAccessor<Q, F> {
    fn clone(&self) -> Self {
        Self {
            archetypes_map: self.archetypes_map.clone(),
            _marker: PhantomData,
        }
    }
}

impl<Q: QueryData, F: QueryFilter> ParallelQueryAccessor<Q, F> {
    pub fn scope<Func, R>(&self, f: Func) -> R
    where
        Func: for<'q, 'a> FnOnce(Query<'q, 'a, Q, F>) -> R,
    {
        let query = Query::new(&self.archetypes_map);
        f(query)
    }
}

unsafe impl<Q: QueryData, F: QueryFilter> Send for ParallelQueryAccessor<Q, F> {}
unsafe impl<Q: QueryData, F: QueryFilter> Sync for ParallelQueryAccessor<Q, F> {}

impl<Q: QueryData, F: QueryFilter> SystemParam for ParallelQueryAccessor<Q, F> {
    fn init_access(_system_meta: &mut SystemMeta) {}
    fn get_param(world: &World) -> Self {
        world.get_resource::<ConfigurationContext>().not_ready();
        let mut local_reads = AccessVec::new();
        let mut local_writes = AccessVec::new();
        let mut local_with = AccessVec::new();
        let mut local_without = AccessVec::new();

        Q::collect_access(&mut local_reads, &mut local_writes);
        F::collect_filter(&mut local_with, &mut local_without);
        let has_intra_conflict = local_writes.iter().any(|w| local_reads.contains(w));
        let mut unique_writes = rustc_hash::FxHashSet::default();
        let has_duplicate_writes = local_writes.iter().any(|w| !unique_writes.insert(w));

        if has_intra_conflict || has_duplicate_writes {
            panic!(
                "❌ ECS INTRA-QUERY ARGUMENT CONFLICT in ParallelQueryAccessor: ParallelQueryAccessor<{}, {}> contains internal overlaps!",
                std::any::type_name::<Q>(),
                std::any::type_name::<F>()
            );
        }
        ParallelQueryAccessor {
            archetypes_map: world.archetypes_manager.archetypes.clone(),
            _marker: PhantomData,
        }
    }
}
