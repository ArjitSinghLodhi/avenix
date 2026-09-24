use std::sync::atomic::Ordering;

use crate::entity_registry::REGISTRY_HANDLE_COUNT;

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
