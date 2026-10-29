mod concurrent_registry;

use parking_lot::RwLock;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::{entity::concurrent_registry::ConcurrentRegistry, world::archetypes::ArchetypeId};

/// A lightweight, unique identifier representing an Entity ID within the ECS engine.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Entity {
    registry_index: u32,
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
            let handle_count_ptr =
                ENTITY_HANDLE_COUNT_REGISTRY.get_ptr(self.registry_index() as usize);
            (*handle_count_ptr).0.fetch_add(1, Ordering::Relaxed);
        }
        Entity::new(self.registry_index)
    }
}

impl Drop for Entity {
    fn drop(&mut self) {
        unsafe {
            let handle_count_ptr =
                ENTITY_HANDLE_COUNT_REGISTRY.get_ptr(self.registry_index() as usize);
            (*handle_count_ptr).0.fetch_sub(1, Ordering::Release);
        }
    }
}

pub(crate) struct RegistryData {
    archetype_id: ArchetypeId,
    index: u32,
}

impl RegistryData {
    pub(crate) fn new(archetype_id: ArchetypeId, index: u32) -> Self {
        Self {
            archetype_id,
            index,
        }
    }

    pub(crate) fn archetype_id(&self) -> ArchetypeId {
        self.archetype_id
    }

    pub(crate) fn index(&self) -> u32 {
        self.index
    }

    pub(crate) fn set_archetype_id(&mut self, archetype_id: ArchetypeId) {
        self.archetype_id = archetype_id;
    }

    pub(crate) fn set_index(&mut self, index: u32) {
        self.index = index;
    }
}

pub(crate) struct HandleCount(AtomicU32);

impl HandleCount {
    pub(crate) fn new(count: u32) -> Self {
        Self(AtomicU32::new(count))
    }

    pub(crate) fn get_count(&self) -> u32 {
        self.0.load(Ordering::Acquire)
    }

    pub(crate) fn set_count(&self, count: u32) {
        self.0.store(count, Ordering::Release);
    }

    pub(crate) fn decrement(&self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}

pub(crate) static ENTITY_LOCATION_REGISTRY: RwLock<Vec<RegistryData>> = RwLock::new(Vec::new());
pub(crate) static ENTITY_HANDLE_COUNT_REGISTRY: ConcurrentRegistry<HandleCount> =
    ConcurrentRegistry::new();
