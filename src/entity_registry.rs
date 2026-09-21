use crate::world::archetypes::ArchetypeId;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU32, Ordering};

pub(crate) struct RegistryData {
    pub(crate) archetype_id: ArchetypeId,
    pub(crate) idx: u32,
}

pub(crate) struct UnsafeGlobalRegistryData(UnsafeCell<Vec<RegistryData>>);

unsafe impl Send for UnsafeGlobalRegistryData {}
unsafe impl Sync for UnsafeGlobalRegistryData {}

pub(crate) struct UnsafeGlobalRegistryHandleCount(UnsafeCell<Vec<AtomicU32>>);

unsafe impl Send for UnsafeGlobalRegistryHandleCount {}
unsafe impl Sync for UnsafeGlobalRegistryHandleCount {}

impl UnsafeGlobalRegistryData {
    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        unsafe { (*self.0.get()).len() }
    }

    #[inline(always)]
    pub(crate) fn push(&self, cell: RegistryData) {
        unsafe {
            (*self.0.get()).push(cell);
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn get_ptr(&self, index: usize) -> *const RegistryData {
        unsafe { (*self.0.get()).as_ptr().add(index) }
    }

    #[inline(always)]
    pub(crate) unsafe fn get_mut_ptr(&self, index: usize) -> *mut RegistryData {
        unsafe { (*self.0.get()).as_mut_ptr().add(index) }
    }
}

impl UnsafeGlobalRegistryHandleCount {
    #[inline(always)]
    pub(crate) fn push(&self, count: AtomicU32) {
        unsafe {
            (*self.0.get()).push(count);
        }
    }
    #[inline(always)]
    pub(crate) fn get_count(&self, index: usize) -> u32 {
        unsafe { (*(*self.0.get()).as_ptr().add(index)).load(Ordering::Relaxed) }
    }

    #[inline(always)]
    pub(crate) unsafe fn get_ptr(&self, index: usize) -> *const AtomicU32 {
        unsafe { (*self.0.get()).as_ptr().add(index) }
    }

    #[inline(always)]
    pub(crate) unsafe fn get_mut_ptr(&self, index: usize) -> *mut AtomicU32 {
        unsafe { (*self.0.get()).as_mut_ptr().add(index) }
    }

    #[inline(always)]
    pub(crate) unsafe fn decrement_handle(&self, index: usize) {
        let cell_ptr = unsafe { self.get_ptr(index) };
        unsafe {
            (*cell_ptr).fetch_sub(1, Ordering::Relaxed);
        }
    }
}

pub(crate) static REGISTRY: UnsafeGlobalRegistryData =
    UnsafeGlobalRegistryData(UnsafeCell::new(Vec::new()));

pub(crate) static REGISTRY_HANDLE_COUNT: UnsafeGlobalRegistryHandleCount =
    UnsafeGlobalRegistryHandleCount(UnsafeCell::new(Vec::new()));
