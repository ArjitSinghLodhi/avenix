use crate::ecs::Component;
use crate::query::QueryData;
use crate::query::QueryFilter;
use crate::reactivity::{TRACKED_COMPONENTS, TrackedComponentMeta};
use crate::system::AccessHashSet;
use crate::system::AccessVec;
use crate::world::archetypes::{Archetype, ComponentColumn};
use crate::world::storage::CurrentBufferIdx;
use fxhash::FxBuildHasher;
use indexmap::IndexSet;
use std::any::TypeId;
use std::marker::PhantomData;

pub(crate) fn register_tracked_component<T: Component>() {
    let mut tracked = TRACKED_COMPONENTS.write();
    let component_id = TypeId::of::<T>();

    tracked.insert(
        component_id,
        TrackedComponentMeta {
            component_id,
            marker_id: TypeId::of::<ChangedMarker<T>>(),
            create_marker_column: || ComponentColumn {
                data: Box::new(Vec::<ChangedMarker<T>>::new()),
            },
            push_default_marker: |column| {
                let raw_any = column.data.as_any_mut();
                let vec = raw_any.downcast_mut::<Vec<ChangedMarker<T>>>().unwrap();
                vec.push(ChangedMarker {
                    markers: [false; 2],
                    phantom: PhantomData,
                });
            },
            clear_column_markers: |raw_any| {
                let idx = CurrentBufferIdx::current_write_idx();
                let vec = raw_any.downcast_mut::<Vec<ChangedMarker<T>>>().unwrap();
                vec.iter_mut().for_each(|marker| {
                    marker.markers[idx as usize] = false;
                });
            },
        },
    );
}

#[derive(Clone, Copy)]
pub struct ChangedMarker<T: Component> {
    pub(crate) markers: [bool; 2],
    phantom: PhantomData<T>,
}

impl<T: Component> Component for ChangedMarker<T> {}

/// A query data wrapper that allows systems to inspect the change state of an individual component instance.
///
/// Unlike the [`Changed<T>`] filter, which excludes non-modified entities from the query entirely,
/// `ChangedTracker<T>` allows the entity to match the query while exposing the [`.is_changed()`] method
/// to check its status conditionally.
///
/// # Architecture & Timing
///
/// This tracking relies on the exact same double-buffered, frame-locked system as the filter:
///
/// * **Frame 1:** The target component is mutably modified. The tracking bool is set but remains hidden.
/// * **Frame 2:** [`.is_changed()`] evaluates to `true` globally across this entire frame window.
/// * **Frame 3:** The flag is automatically purged and resets to `false`, regardless of system execution.
///
/// [`.is_changed()`]: ChangedTracker::is_changed
pub struct ChangedTracker<T: Component> {
    changed: bool,
    _marker: PhantomData<T>,
}

impl<T: Component> ChangedTracker<T> {
    /// Returns whether the target component was mutably modified during the previous frame.
    #[inline(always)]
    pub fn is_changed(&self) -> bool {
        self.changed
    }
}

impl<T: Component> QueryData for ChangedTracker<T> {
    type Item<'w> = ChangedTracker<T>;
    type ReadOnlyItem<'w> = ChangedTracker<T>;
    type Fetch = (u8, *const ChangedMarker<T>);

    fn matches(types: &IndexSet<TypeId, FxBuildHasher>) -> bool {
        types.contains(&TypeId::of::<ChangedMarker<T>>())
    }

    fn collect_access(
        reads: &mut AccessVec<std::any::TypeId>,
        _writes: &mut AccessVec<std::any::TypeId>,
    ) {
        reads.push(TypeId::of::<ChangedMarker<T>>());
        register_tracked_component::<T>();
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        let marker_ptr = unsafe { (*archetype.fetch_column_raw::<ChangedMarker<T>>()).as_ptr() };
        let current_read_idx = CurrentBufferIdx::current_read_idx();

        (current_read_idx, marker_ptr)
    }

    unsafe fn fetch_read_only<'w>(fetch: Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        let marker_ref = unsafe { &(*fetch.1.add(index)) };
        let changed = marker_ref.markers[fetch.0 as usize];
        ChangedTracker {
            changed,
            _marker: PhantomData,
        }
    }

    unsafe fn fetch_mut<'w>(fetch: Self::Fetch, index: usize) -> Self::Item<'w> {
        let marker_ref = unsafe { &(*fetch.1.add(index)) };
        let changed = marker_ref.markers[fetch.0 as usize];
        ChangedTracker {
            changed,
            _marker: PhantomData,
        }
    }
}

/// A query filter that matches components of type `T` that were mutably modified during the previous frame.
///
/// # Architecture & Timing
///
/// Change detection in Avenix is globally double-buffered and strictly time-bound. It is **not** tracking
/// individual systems or reading history. The visibility window follows a strict 3-frame lifecycle:
///
/// * **Frame 1 (Modification):** The component is mutably accessed or modified. The change is registered internally but is **not** yet visible to queries.
/// * **Frame 2 (Detection Window):** The change becomes globally visible. Any system running in this frame filtering for `Changed<T>` will detect it.
/// * **Frame 3 (Purge):** The change state is unconditionally cleared.
///
/// Regardless of whether a system ran or read the data, the detection flag will never last for more than exactly one frame.
pub struct Changed<T: Component>(std::marker::PhantomData<T>);

impl<T: Component> QueryFilter for Changed<T> {
    fn matches(types: &AccessHashSet<TypeId>) -> bool {
        types.contains(&TypeId::of::<ChangedMarker<T>>())
    }

    fn collect_filter(withs: &mut AccessVec<TypeId>, _withouts: &mut AccessVec<TypeId>) {
        withs.push(TypeId::of::<ChangedMarker<T>>());
        register_tracked_component::<T>();
    }

    fn filter_indices(archetype: &Archetype, indices: &mut Vec<usize>) {
        let marker_ptr = unsafe { (*archetype.fetch_column_raw::<ChangedMarker<T>>()).as_ptr() };
        let current_read_idx = CurrentBufferIdx::current_read_idx();

        indices.retain(|&idx| unsafe { &*marker_ptr.add(idx) }.markers[current_read_idx as usize]);
    }
}

pub struct Mut<'w, T: Component> {
    pub(crate) value: *mut T,
    pub(crate) marker: *mut ChangedMarker<T>,
    pub(crate) current_write_idx: u8,
    pub(crate) should_modify: bool,
    pub(crate) _marker: std::marker::PhantomData<&'w mut T>,
}

impl<'w, T: Component> Mut<'w, T> {
    #[inline(always)]
    pub fn into_raw_mut(self) -> &'w mut T {
        unsafe { &mut *self.value }
    }
    #[inline(always)]
    pub fn bypass_change_detection(&mut self) -> &mut T {
        unsafe { &mut *self.value }
    }

    #[inline(always)]
    pub fn trigger_change_detection(&mut self) {
        if self.should_modify {
            unsafe {
                (*self.marker).markers[self.current_write_idx as usize] = true;
            }
        }
    }

    #[inline(always)]
    pub fn as_ref(&self) -> &T {
        unsafe { &*self.value }
    }
}

unsafe impl<'w, T: Component + Send> Send for Mut<'w, T> {}
unsafe impl<'w, T: Component + Sync> Sync for Mut<'w, T> {}

impl<'w, T: Component> std::ops::Deref for Mut<'w, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.value }
    }
}

impl<'w, T: Component> std::ops::DerefMut for Mut<'w, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            if self.should_modify {
                (*self.marker).markers[self.current_write_idx as usize] = true;
            }
            &mut *self.value
        }
    }
}

impl<'w, T: std::fmt::Debug + Component> std::fmt::Debug for Mut<'w, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe { std::fmt::Debug::fmt(&*self.value, f) }
    }
}

impl<'w, T: std::fmt::Display + Component> std::fmt::Display for Mut<'w, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe { std::fmt::Display::fmt(&*self.value, f) }
    }
}
