use std::{marker::PhantomData, sync::Arc};

use dashmap::DashMap;
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
/// # Safety and Undefined Behaviour
/// To prevent **Undefined Behaviour (UB)** and memory unsafety within the engine's internal query mechanics,
/// you must strictly adhere to the following rules while this parallel handle is active:
/// * **No Handle Cloning:** Do not clone entity handles.
/// * **No Random Query Lookups:** Do not perform random lookups using queries.
///
/// **Exception:** These actions are only permissible if you explicitly guarantee that absolutely no entity handle clones
/// or query-based random lookups can occur while commands are being applied at the end of the frame.
///
/// # Deadlock Safety Rules
/// Because the underlying storage utilizes granular, column-level `RwLocks` to enable simultaneous
/// multi-threaded table access, nesting parallel query scopes incorrectly on the *same thread* will freeze execution:
/// * **Same-Thread Deadlocks:** Nesting parallel query scopes incorrectly on the *same thread* will freeze execution.
/// * **Overlapping Access:** Opening a mutable query scope while an active scope (mutable or immutable)
///   targets overlapping components on that same thread will trigger a deadlock.
///
/// [`.get_par_query_accessor()`]: crate::app::App::get_par_query_accessor
pub struct ParallelQueryAccessor<Q: QueryData, F: QueryFilter = EmptyQueryFilter> {
    pub(crate) archetypes_map: Arc<DashMap<ArchetypeId, Archetype, FxBuildHasher>>,
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
