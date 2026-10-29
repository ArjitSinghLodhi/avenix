use crate::entity::Entity;
use std::alloc::{Layout, alloc_zeroed, dealloc, handle_alloc_error};
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

const LEAF_SIZE: usize = 1024;
const MID_SIZE: usize = 64;
const ROOT_SIZE: usize = 1024;

pub(crate) struct ConcurrentBitSet {
    root: [AtomicPtr<AtomicPtr<AtomicU64>>; ROOT_SIZE],
}

unsafe impl Send for ConcurrentBitSet {}
unsafe impl Sync for ConcurrentBitSet {}

impl ConcurrentBitSet {
    pub(crate) const fn new() -> Self {
        let root: [AtomicPtr<AtomicPtr<AtomicU64>>; ROOT_SIZE] = {
            unsafe {
                std::mem::transmute::<
                    [AtomicPtr<std::ffi::c_void>; ROOT_SIZE],
                    [AtomicPtr<AtomicPtr<AtomicU64>>; ROOT_SIZE],
                >([const { AtomicPtr::new(ptr::null_mut()) }; ROOT_SIZE])
            }
        };

        Self { root }
    }

    pub(crate) fn insert(&self, item: Entity) -> bool {
        let value: u32 = item.registry_index();
        std::mem::forget(item);
        let word_idx = (value >> 6) as usize;
        let bit_shift = value & 63;
        let mask = 1 << bit_shift;

        let root_idx = (word_idx >> 16) & 1023;
        let mid_idx = (word_idx >> 10) & 63;
        let leaf_idx = word_idx & 1023;

        let mut mid_table_ptr = self.root[root_idx].load(Ordering::Acquire);
        if mid_table_ptr.is_null() {
            let layout = Layout::array::<AtomicPtr<AtomicU64>>(MID_SIZE).unwrap();
            let new_mid = unsafe { alloc_zeroed(layout) as *mut AtomicPtr<AtomicU64> };
            if new_mid.is_null() {
                handle_alloc_error(layout);
            }

            if let Err(actual) = self.root[root_idx].compare_exchange(
                ptr::null_mut(),
                new_mid,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                unsafe {
                    dealloc(new_mid as *mut u8, layout);
                }
                mid_table_ptr = actual;
            } else {
                mid_table_ptr = new_mid;
            }
        }

        let leaf_atomic_ref = unsafe { &*mid_table_ptr.add(mid_idx) };
        let mut leaf_chunk_ptr = leaf_atomic_ref.load(Ordering::Acquire);
        if leaf_chunk_ptr.is_null() {
            let layout = Layout::array::<AtomicU64>(LEAF_SIZE).unwrap();
            let new_leaf = unsafe { alloc_zeroed(layout) as *mut AtomicU64 };
            if new_leaf.is_null() {
                handle_alloc_error(layout);
            }

            if let Err(actual) = leaf_atomic_ref.compare_exchange(
                ptr::null_mut(),
                new_leaf,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                unsafe {
                    dealloc(new_leaf as *mut u8, layout);
                }
                leaf_chunk_ptr = actual;
            } else {
                leaf_chunk_ptr = new_leaf;
            }
        }

        let word_ref = unsafe { &*leaf_chunk_ptr.add(leaf_idx) };
        let previous = word_ref.fetch_or(mask, Ordering::AcqRel);
        (previous & mask) != 0
    }

    pub(crate) fn contains(&self, item: &Entity) -> bool {
        let value: u32 = item.registry_index();
        let word_idx = (value >> 6) as usize;
        let bit_shift = value & 63;
        let mask = 1 << bit_shift;

        let root_idx = (word_idx >> 16) & 1023;
        let mid_idx = (word_idx >> 10) & 63;
        let leaf_idx = word_idx & 1023;

        let mid_table_ptr = self.root[root_idx].load(Ordering::Acquire);
        if mid_table_ptr.is_null() {
            return false;
        }

        let leaf_atomic_ref = unsafe { &*mid_table_ptr.add(mid_idx) };
        let leaf_chunk_ptr = leaf_atomic_ref.load(Ordering::Acquire);
        if leaf_chunk_ptr.is_null() {
            return false;
        }

        let word_ref = unsafe { &*leaf_chunk_ptr.add(leaf_idx) };
        let current = word_ref.load(Ordering::Relaxed);
        (current & mask) != 0
    }

    pub(crate) fn remove(&self, item: &Entity) -> bool {
        let value: u32 = item.registry_index();
        let word_idx = (value >> 6) as usize;
        let bit_shift = value & 63;
        let mask = 1 << bit_shift;

        let root_idx = (word_idx >> 16) & 1023;
        let mid_idx = (word_idx >> 10) & 63;
        let leaf_idx = word_idx & 1023;

        let mid_table_ptr = self.root[root_idx].load(Ordering::Acquire);
        if mid_table_ptr.is_null() {
            return false;
        }

        let leaf_atomic_ref = unsafe { &*mid_table_ptr.add(mid_idx) };
        let leaf_chunk_ptr = leaf_atomic_ref.load(Ordering::Acquire);
        if leaf_chunk_ptr.is_null() {
            return false;
        }

        let word_ref = unsafe { &*leaf_chunk_ptr.add(leaf_idx) };
        let previous = word_ref.fetch_and(!mask, Ordering::AcqRel);
        (previous & mask) != 0
    }

    pub(crate) fn is_empty(&self) -> bool {
        for root_atomic in self.root.iter() {
            let mid_table_ptr = root_atomic.load(Ordering::Acquire);
            if !mid_table_ptr.is_null() {
                unsafe {
                    for mid_idx in 0..MID_SIZE {
                        let leaf_atomic_ptr = &*mid_table_ptr.add(mid_idx);
                        let leaf_chunk_ptr = leaf_atomic_ptr.load(Ordering::Acquire);
                        if !leaf_chunk_ptr.is_null() {
                            for leaf_idx in 0..LEAF_SIZE {
                                let word_atomic = &*leaf_chunk_ptr.add(leaf_idx);
                                if word_atomic.load(Ordering::Relaxed) != 0 {
                                    return false;
                                }
                            }
                        }
                    }
                }
            }
        }
        true
    }

    pub(crate) fn drain<F>(&self, mut f: F)
    where
        F: FnMut(Entity),
    {
        for (root_idx, root_atomic) in self.root.iter().enumerate() {
            let mid_table_ptr = root_atomic.load(Ordering::Acquire);
            if !mid_table_ptr.is_null() {
                unsafe {
                    for mid_idx in 0..MID_SIZE {
                        let leaf_atomic_ptr = &*mid_table_ptr.add(mid_idx);
                        let leaf_chunk_ptr = leaf_atomic_ptr.load(Ordering::Acquire);
                        if !leaf_chunk_ptr.is_null() {
                            for leaf_idx in 0..LEAF_SIZE {
                                let word_atomic = &*leaf_chunk_ptr.add(leaf_idx);
                                let mut bits = word_atomic.swap(0, Ordering::AcqRel);
                                if bits != 0 {
                                    let base_value =
                                        (((root_idx << 16) | (mid_idx << 10) | leaf_idx) << 6)
                                            as u32;
                                    while bits != 0 {
                                        let bit_shift = bits.trailing_zeros();
                                        f(Entity::new(base_value + bit_shift));
                                        bits &= bits - 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Drop for ConcurrentBitSet {
    fn drop(&mut self) {
        let mid_layout = Layout::array::<AtomicPtr<AtomicU64>>(MID_SIZE).unwrap();
        let leaf_layout = Layout::array::<AtomicU64>(LEAF_SIZE).unwrap();

        for root_atomic in &mut self.root {
            let mid_table_ptr = *root_atomic.get_mut();
            if !mid_table_ptr.is_null() {
                unsafe {
                    for mid_idx in 0..MID_SIZE {
                        let leaf_atomic_ptr = &mut *mid_table_ptr.add(mid_idx);
                        let leaf_chunk_ptr = *leaf_atomic_ptr.get_mut();
                        if !leaf_chunk_ptr.is_null() {
                            dealloc(leaf_chunk_ptr as *mut u8, leaf_layout);
                        }
                    }
                    dealloc(mid_table_ptr as *mut u8, mid_layout);
                }
            }
        }
    }
}
