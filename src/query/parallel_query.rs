use std::{marker::PhantomData, sync::Arc};

use dashmap::DashMap;
use parking_lot::Mutex;
use rustc_hash::FxBuildHasher;

use crate::{
    extensions::Archetype,
    query::{EmptyQueryFilter, Query, QueryData, QueryFilter},
    world::archetypes::ArchetypeId,
};

/// A thread-safe, clonable handle providing detached remote access to the engine's archetype data blocks.
///
/// `ParallelQueryAccessor` can be passed to external or background worker threads, allowing them to
/// concurrently read or modify component data arrays outside the main execution path. It can be obtained
/// directly from the application layer via [`.get_par_query_accessor()`] before building the app.
///
/// # Initialization Requirement
/// Unlike other parallel handles, this handle must be initialized prior to app compilation. This ensures the
/// engine can automatically register and configure the underlying demand-driven component tracking.
///
/// # Thread Safety
/// Standard concurrent reads and writes for component columns are completely safe without any extra rules.
/// For specific rules regarding cloning `Entity` handles, lookups, or using [`query.get()`] / [`view.get()`] out-of-band,
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
/// [`.get_par_query_accessor()`]: crate::app::App::get_par_query_accessor
/// [`Entity`]: crate::entity::Entity
/// [`query.get()`]: crate::query::Query::get
/// [`view.get()`]: crate::query::QueryArchetypeView::get
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
