use std::sync::atomic::Ordering;

use crate::entity_registry::REGISTRY_HANDLE_COUNT;

/// A lightweight, unique identifier representing an Entity ID within the ECS engine.
///
/// `Entity` handles can be safely passed to external or background worker threads—such as via a
/// [`ParallelQueryAccessor`]—allowing out-of-band data processing outside the main ECS execution path.
///
/// # Safety and Undefined Behavior (UB)
/// To prevent memory corruption and internal query desynchronization, you must strictly adhere to the
/// following execution rules when working with entity handles out-of-band:
/// * **No Cloning While Commands Execute:** You must never call [`.clone()`][Self::clone] on an `Entity`
///   handle while the ECS world is actively executing and applying commands.
/// * **No Random Lookups While Commands Execute:** You must never perform random lookups using query
///   methods like [`query.get()`] or [`view.get()`] while the ECS world is actively executing and applying commands.
///
/// **Crucial Distinction:** These restrictions apply **strictly and only** when manipulating or querying
/// via `Entity` handles themselves. Out-of-band access to component columns or other archetype data blocks
/// is completely exempt from these rules and remains safe to access even while commands are executing.
///
/// [`ParallelQueryAccessor`]: crate::query::parallel_query::ParallelQueryAccessor
/// [`query.get()`]: crate::query::Query::get
/// [`view.get()`]: crate::query::QueryArchetypeView::get
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    pub(crate) registry_index: u32,
}

impl Entity {
    pub(crate) fn new(registry_index: u32) -> Self {
        Self { registry_index }
    }
    #[inline(always)]
    pub fn registry_index(&self) -> u32 {
        self.registry_index
    }
}

impl Clone for Entity {
    fn clone(&self) -> Self {
        unsafe {
            let atomic_ptr = REGISTRY_HANDLE_COUNT.get_ptr(self.registry_index() as usize);
            (*atomic_ptr).fetch_add(1, Ordering::Relaxed);
        }
        Entity::new(self.registry_index)
    }
}

impl Drop for Entity {
    fn drop(&mut self) {
        unsafe {
            let atomic_ptr = REGISTRY_HANDLE_COUNT.get_ptr(self.registry_index() as usize);
            (*atomic_ptr).fetch_sub(1, Ordering::Relaxed);
        }
    }
}
