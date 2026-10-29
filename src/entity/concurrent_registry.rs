use std::alloc::{Layout, alloc_zeroed};
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

const LEAF_SHIFT: u32 = 12;
const LEAF_SIZE: usize = 1 << LEAF_SHIFT;
const LEAF_MASK: usize = LEAF_SIZE - 1;

const MID_SHIFT: u32 = 10;
const MID_SIZE: usize = 1 << MID_SHIFT;
const MID_MASK: usize = MID_SIZE - 1;

const ROOT_SIZE: usize = 1024;

pub(crate) struct ConcurrentRegistry<T> {
    root: [AtomicPtr<AtomicPtr<T>>; ROOT_SIZE],
    len: AtomicU32,
}

unsafe impl<T> Send for ConcurrentRegistry<T> {}
unsafe impl<T> Sync for ConcurrentRegistry<T> {}

impl<T> ConcurrentRegistry<T> {
    pub(crate) const fn new() -> Self {
        let root: [AtomicPtr<AtomicPtr<T>>; ROOT_SIZE] = {
            unsafe {
                std::mem::transmute::<
                    [AtomicPtr<std::ffi::c_void>; ROOT_SIZE],
                    [AtomicPtr<AtomicPtr<T>>; ROOT_SIZE],
                >([const { AtomicPtr::new(ptr::null_mut()) }; ROOT_SIZE])
            }
        };

        Self {
            root,
            len: AtomicU32::new(0),
        }
    }

    #[inline(always)]
    pub(crate) unsafe fn get_ptr(&self, index: usize) -> *const T {
        let root_idx = (index >> (LEAF_SHIFT + MID_SHIFT)) & (ROOT_SIZE - 1);
        let mid_idx = (index >> LEAF_SHIFT) & MID_MASK;
        let leaf_idx = index & LEAF_MASK;

        let mid_table_ptr = self.root[root_idx].load(Ordering::Acquire);
        if mid_table_ptr.is_null() {
            return ptr::null_mut();
        }

        let leaf_chunk_ptr = unsafe { (*mid_table_ptr.add(mid_idx)).load(Ordering::Acquire) };
        if leaf_chunk_ptr.is_null() {
            return ptr::null_mut();
        }

        unsafe { leaf_chunk_ptr.add(leaf_idx) }
    }

    pub(crate) fn push(&self, value: T) -> u32 {
        let assigned_idx = self.len.fetch_add(1, Ordering::Relaxed) as usize;

        let root_idx = (assigned_idx >> (LEAF_SHIFT + MID_SHIFT)) & (ROOT_SIZE - 1);
        let mid_idx = (assigned_idx >> LEAF_SHIFT) & MID_MASK;
        let leaf_idx = assigned_idx & LEAF_MASK;

        let mut mid_table_ptr = self.root[root_idx].load(Ordering::Acquire);
        if mid_table_ptr.is_null() {
            let layout = Layout::array::<AtomicPtr<T>>(MID_SIZE).unwrap();
            let new_mid = unsafe { alloc_zeroed(layout) as *mut AtomicPtr<T> };

            if let Err(actual) = self.root[root_idx].compare_exchange(
                ptr::null_mut(),
                new_mid,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                unsafe {
                    std::alloc::dealloc(new_mid as *mut u8, layout);
                }
                mid_table_ptr = actual;
            } else {
                mid_table_ptr = new_mid;
            }
        }

        let leaf_atomic_ref = unsafe { &*mid_table_ptr.add(mid_idx) };
        let mut leaf_chunk_ptr = leaf_atomic_ref.load(Ordering::Acquire);
        if leaf_chunk_ptr.is_null() {
            let layout = Layout::array::<T>(LEAF_SIZE).unwrap();
            let new_leaf = unsafe { alloc_zeroed(layout) as *mut T };

            if let Err(actual) = leaf_atomic_ref.compare_exchange(
                ptr::null_mut(),
                new_leaf,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                unsafe {
                    std::alloc::dealloc(new_leaf as *mut u8, layout);
                }
                leaf_chunk_ptr = actual;
            } else {
                leaf_chunk_ptr = new_leaf;
            }
        }

        unsafe {
            leaf_chunk_ptr.add(leaf_idx).write(value);
        }

        assigned_idx as u32
    }
}

impl<T> Drop for ConcurrentRegistry<T> {
    fn drop(&mut self) {
        let mid_layout = Layout::array::<AtomicPtr<T>>(MID_SIZE).unwrap();
        let leaf_layout = Layout::array::<T>(LEAF_SIZE).unwrap();

        for root_ptr in &self.root {
            let mid_table_ptr = root_ptr.load(Ordering::Relaxed);
            if !mid_table_ptr.is_null() {
                unsafe {
                    for mid_idx in 0..MID_SIZE {
                        let leaf_chunk_ptr = (*mid_table_ptr.add(mid_idx)).load(Ordering::Relaxed);
                        if !leaf_chunk_ptr.is_null() {
                            std::alloc::dealloc(leaf_chunk_ptr as *mut u8, leaf_layout);
                        }
                    }
                    std::alloc::dealloc(mid_table_ptr as *mut u8, mid_layout);
                }
            }
        }
    }
}
