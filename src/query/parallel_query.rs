use std::{marker::PhantomData, sync::Arc};

use dashmap::DashMap;
use rustc_hash::FxBuildHasher;

use crate::{
    extensions::Archetype,
    query::{EmptyQueryFilter, Query, QueryData, QueryFilter},
    world::archetypes::ArchetypeId,
};

/// A thread-safe, thread-clonable handle that acts as a detached remote to query the engine's archetype data blocks.
///
/// `ParallelQueryAccessor` can be passed to external or background worker threads, allowing them to
/// concurrently read or modify component data arrays outside the main execution path. It can be obtained
/// directly from the application layer via [`.get_par_query_accessor()`] before building the app.
///
/// # Rules
///
/// Because the underlying storage utilizes granular, column-level `RwLocks` to enable simultaneous
/// multi-threaded table reading, nesting parallel query scopes incorrectly on the *same thread* will trigger a deadlock.
/// Specifically, opening a mutable query scope while an immutable or mutable query scope targeting overlapping
/// components is already active within that thread will freeze execution.
///
/// Unlike other parallel handles, this handle must be initialized prior to app compilation to automatically
/// register demand-driven component tracking configurations behind the scenes.
///
/// [`.get_par_query_accessor()`]: crate::app::App::get_par_query_accessor
pub struct ParallelQueryAccessor<Q: QueryData, F: QueryFilter = EmptyQueryFilter> {
    pub(crate) archetypes_map: Arc<DashMap<ArchetypeId, Archetype, FxBuildHasher>>,
    pub(crate) _marker: PhantomData<(Q, F)>,
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
