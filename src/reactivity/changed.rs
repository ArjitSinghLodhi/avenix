use crate::ecs::Component;
use crate::extensions::ComponentColumnRead;
use crate::query::QueryData;
use crate::query::QueryFilter;
use crate::reactivity::verify_app_not_build_yet;
use crate::reactivity::{TRACKED_COMPONENTS, TrackedComponentMeta};
use crate::system::AccessVec;
use crate::world::archetypes::{Archetype, ComponentColumn};
use parking_lot::RwLock;
use std::any::TypeId;
use std::marker::PhantomData;

pub(crate) fn register_changed_tracked_component<T: Component>() {
    let _lock = verify_app_not_build_yet();
    let mut tracked = TRACKED_COMPONENTS.lock();
    let component_id = TypeId::of::<T>();
    let marker_id = TypeId::of::<ChangedMarker<T>>();

    let meta_list = tracked.entry(component_id).or_default();

    if meta_list.iter().any(|m| m.marker_id == marker_id) {
        return;
    }

    meta_list.push(TrackedComponentMeta {
        marker_id,
        create_marker_column: || ComponentColumn {
            data: RwLock::new(Box::new(Vec::<ChangedMarker<T>>::new())),
        },
        push_default_marker: |column| {
            let mut guard = column.data.write();
            let raw_any = guard.as_any_mut();
            let vec = raw_any.downcast_mut::<Vec<ChangedMarker<T>>>().unwrap();
            vec.push(ChangedMarker {
                read: false,
                write: false,
                phantom: PhantomData,
            });
        },
        clear_column_markers: |raw_any| {
            let vec = raw_any.downcast_mut::<Vec<ChangedMarker<T>>>().unwrap();
            vec.iter_mut().for_each(|marker| {
                marker.read = marker.write;
                marker.write = false;
            });
        },
    });
}

#[derive(Clone, Copy)]
pub struct ChangedMarker<T: Component> {
    read: bool,
    write: bool,
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
    type Fetch<'w> = ComponentColumnRead<'w, ChangedMarker<T>>;

    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }

    fn collect_access(
        reads: &mut AccessVec<std::any::TypeId>,
        _writes: &mut AccessVec<std::any::TypeId>,
    ) {
        reads.push(TypeId::of::<ChangedMarker<T>>());
        register_changed_tracked_component::<T>();
    }

    unsafe fn init_fetch<'w>(archetype: &'w Archetype) -> Self::Fetch<'w> {
        archetype.get_column::<ChangedMarker<T>>()
    }

    unsafe fn fetch_read_only<'w, 'b>(
        fetch: &Self::Fetch<'b>,
        index: usize,
    ) -> Self::ReadOnlyItem<'w> {
        let marker_ref = unsafe { &(*fetch.as_ptr().add(index)) };
        let changed = marker_ref.read;
        ChangedTracker {
            changed,
            _marker: PhantomData,
        }
    }

    unsafe fn fetch_mut<'w, 'b>(fetch: &Self::Fetch<'b>, index: usize) -> Self::Item<'w> {
        let marker_ref = unsafe { &(*fetch.as_ptr().add(index)) };
        let changed = marker_ref.read;
        ChangedTracker {
            changed,
            _marker: PhantomData,
        }
    }
}

pub struct ChangedRefMut<'w, T: Component> {
    value: Mut<'w, T>,
    changed: bool,
}

pub struct ChangedRef<'w, T: Component> {
    value: &'w T,
    changed: bool,
}

impl<'w, T: Component> ChangedRefMut<'w, T> {
    #[inline(always)]
    pub fn is_changed(&self) -> bool {
        self.changed
    }
    #[inline(always)]
    pub fn bypass_change_detection(&mut self) -> &mut T {
        self.value.bypass_change_detection()
    }

    #[inline(always)]
    pub fn trigger_change_detection(&mut self) {
        self.value.trigger_change_detection();
    }
}

impl<'w, T: Component> ChangedRef<'w, T> {
    #[inline(always)]
    pub fn is_changed(&self) -> bool {
        self.changed
    }
}

impl<'w, T: Component> std::ops::Deref for ChangedRefMut<'w, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.value.value }
    }
}

impl<'w, T: Component> std::ops::Deref for ChangedRef<'w, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'w, T: Component> std::ops::DerefMut for ChangedRefMut<'w, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value.trigger_change_detection();
        unsafe { &mut *self.value.value }
    }
}

impl<T: Component> QueryData for ChangedRefMut<'_, T> {
    type Item<'w> = ChangedRefMut<'w, T>;
    type ReadOnlyItem<'w> = ChangedRef<'w, T>;
    type Fetch<'w> = (
        <&'w mut T as QueryData>::Fetch<'w>,
        <ChangedTracker<T> as QueryData>::Fetch<'w>,
    );

    fn matches(archetype: &Archetype) -> bool {
        <&mut T as QueryData>::matches(archetype)
    }

    fn collect_access(
        reads: &mut AccessVec<std::any::TypeId>,
        writes: &mut AccessVec<std::any::TypeId>,
    ) {
        <&mut T as QueryData>::collect_access(reads, writes);
        <ChangedTracker<T> as QueryData>::collect_access(reads, writes);
    }

    unsafe fn init_fetch<'w>(archetype: &'w Archetype) -> Self::Fetch<'w> {
        unsafe {
            (
                <&mut T as QueryData>::init_fetch(archetype),
                <ChangedTracker<T> as QueryData>::init_fetch(archetype),
            )
        }
    }

    #[inline(always)]
    unsafe fn fetch_mut<'w, 'b>(fetch: &Self::Fetch<'b>, index: usize) -> Self::Item<'w> {
        let mut_item = unsafe { <&mut T as QueryData>::fetch_mut(&fetch.0, index) };
        let tracker = unsafe { <ChangedTracker<T> as QueryData>::fetch_mut(&fetch.1, index) };

        ChangedRefMut {
            value: mut_item,
            changed: tracker.changed,
        }
    }

    unsafe fn fetch_read_only<'w, 'b>(
        fetch: &Self::Fetch<'b>,
        index: usize,
    ) -> Self::ReadOnlyItem<'w> {
        let read_only_item = unsafe { <&mut T as QueryData>::fetch_read_only(&fetch.0, index) };
        let tracker = unsafe { <ChangedTracker<T> as QueryData>::fetch_read_only(&fetch.1, index) };

        ChangedRef {
            value: read_only_item,
            changed: tracker.changed,
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
    type FilterData<'w> = ComponentColumnRead<'w, ChangedMarker<T>>;
    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }

    fn init_filter_data<'w>(archetype: &Archetype) -> Self::FilterData<'w> {
        archetype.get_column::<ChangedMarker<T>>()
    }

    fn matches_row<'w>(filter_data: &Self::FilterData<'w>, row_idx: usize) -> bool {
        unsafe { (*(filter_data.as_ptr().add(row_idx))).read }
    }

    fn collect_filter(withs: &mut AccessVec<TypeId>, _withouts: &mut AccessVec<TypeId>) {
        withs.push(TypeId::of::<ChangedMarker<T>>());
        register_changed_tracked_component::<T>();
    }
}

pub struct Mut<'w, T: Component> {
    pub(crate) value: *mut T,
    pub(crate) marker: *mut ChangedMarker<T>,
    pub(crate) _marker: std::marker::PhantomData<&'w mut T>,
}

impl<'w, T: Component> Mut<'w, T> {
    #[inline(always)]
    pub fn bypass_change_detection(&mut self) -> &mut T {
        unsafe { &mut *self.value }
    }

    #[inline(always)]
    pub fn trigger_change_detection(&mut self) {
        if !self.marker.is_null() {
            unsafe {
                (*self.marker).write = true;
            }
        }
    }
}

impl<'w, T: Component> AsRef<T> for Mut<'w, T> {
    fn as_ref(&self) -> &T {
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
            self.trigger_change_detection();
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
