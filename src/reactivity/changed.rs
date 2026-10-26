use crate::ecs::component::Component;
use crate::query::QueryData;
use crate::query::QueryFilter;
use crate::reactivity::verify_app_not_built_yet;
use crate::reactivity::{TRACKED_COMPONENTS, TrackedComponentMeta};
use crate::system::AccessVec;
use crate::world::archetypes::ComponentColumnRead;
use crate::world::archetypes::{Archetype, ComponentColumn};
use parking_lot::RwLock;
use std::any::TypeId;
use std::any::type_name;
use std::marker::PhantomData;

pub(crate) fn register_changed_tracked_component<T: Component>() {
    let mut tracked = TRACKED_COMPONENTS.lock();
    let component_id = TypeId::of::<T>();
    let marker_id = TypeId::of::<ChangedMarker<T>>();

    let meta_list = tracked.entry(component_id).or_default();

    if meta_list.iter().any(|m| m.marker_id == marker_id) {
        return;
    }
    let _lock = verify_app_not_built_yet(type_name::<ChangedMarker<T>>());
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
                write: true,
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
    pub(crate) write: bool,
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
/// This tracking relies on the exact same double-buffered, frame-locked system as the filter.
/// For detailed breakdown of the 3-frame lifecycle timing, command application phases, and
/// initial spawn behavior, see the documentation for [`Changed<T>`].
///
/// [`.is_changed()`]: ChangedTracker::is_changed
pub struct ChangedTracker<T: Component> {
    changed: bool,
    _marker: PhantomData<T>,
}

impl<T: Component> ChangedTracker<T> {
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
        archetype.column::<ChangedMarker<T>>()
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
    type Fetch<'w> = <&'w mut T as QueryData>::Fetch<'w>;

    fn matches(archetype: &Archetype) -> bool {
        <&mut T as QueryData>::matches(archetype)
    }

    fn collect_access(
        reads: &mut AccessVec<std::any::TypeId>,
        writes: &mut AccessVec<std::any::TypeId>,
    ) {
        <&mut T as QueryData>::collect_access(reads, writes);
        register_changed_tracked_component::<T>();
    }

    unsafe fn init_fetch<'w>(archetype: &'w Archetype) -> Self::Fetch<'w> {
        unsafe { <&mut T as QueryData>::init_fetch(archetype) }
    }

    #[inline(always)]
    unsafe fn fetch_mut<'w, 'b>(fetch: &Self::Fetch<'b>, index: usize) -> Self::Item<'w> {
        let mut_item = unsafe { <&mut T as QueryData>::fetch_mut(fetch, index) };
        let changed = unsafe { (&*mut_item.changed_marker).read };
        ChangedRefMut {
            value: mut_item,
            changed,
        }
    }

    unsafe fn fetch_read_only<'w, 'b>(
        fetch: &Self::Fetch<'b>,
        index: usize,
    ) -> Self::ReadOnlyItem<'w> {
        let read_only_item = unsafe { <&mut T as QueryData>::fetch_read_only(fetch, index) };
        let changed = unsafe { (&*fetch.1.as_ref().unwrap().as_ptr().add(index)).read };
        let tracker = ChangedTracker::<T> {
            changed,
            _marker: PhantomData,
        };

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
///
/// # Structural Mutations & Spawning
///
/// Spawning a new entity with a component or inserting a component via commands acts as an initial mutation event.
/// This structural modification triggers change tracking identically to a manual component mutation:
///
/// 1. **Command Queuing:** You call `.spawn()` or `.insert()` to queue a component modification command.
/// 2. **Immediate Application:** The command is flushed and applied to the world during that same frame's execution phase.
/// 3. **Next-Frame Visibility:** The tracking markers are flagged immediately following application, making `Changed<T>` evaluate to `true` across the entirety of the next frame.
pub struct Changed<T: Component>(std::marker::PhantomData<T>);

impl<T: Component> QueryFilter for Changed<T> {
    type FilterData<'w> = ComponentColumnRead<'w, ChangedMarker<T>>;
    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }

    fn init_filter_data<'w>(archetype: &Archetype) -> Self::FilterData<'w> {
        archetype.column::<ChangedMarker<T>>()
    }

    fn matches_row<'w>(filter_data: &Self::FilterData<'w>, row_idx: usize) -> bool {
        unsafe { (*(filter_data.as_ptr().add(row_idx))).read }
    }

    fn collect_filter(withs: &mut AccessVec<TypeId>, _withouts: &mut AccessVec<TypeId>) {
        withs.push(TypeId::of::<ChangedMarker<T>>());
        register_changed_tracked_component::<T>();
    }

    fn collect_filter_access(reads: &mut AccessVec<TypeId>, _writes: &mut AccessVec<TypeId>) {
        reads.push(TypeId::of::<ChangedMarker<T>>());
    }
}

pub struct Mut<'w, T: Component> {
    pub(crate) value: *mut T,
    pub(crate) changed_marker: *mut ChangedMarker<T>,
    pub(crate) _marker: std::marker::PhantomData<&'w mut T>,
}

impl<'w, T: Component> Mut<'w, T> {
    #[inline(always)]
    pub fn bypass_change_detection(&mut self) -> &mut T {
        unsafe { &mut *self.value }
    }

    #[inline(always)]
    pub fn trigger_change_detection(&mut self) {
        if !self.changed_marker.is_null() {
            unsafe {
                (*self.changed_marker).write = true;
            }
        }
    }
}

impl<'w, T: Component> AsRef<T> for Mut<'w, T> {
    fn as_ref(&self) -> &T {
        unsafe { &*self.value }
    }
}

unsafe impl<'w, T: Component> Send for Mut<'w, T> {}
unsafe impl<'w, T: Component> Sync for Mut<'w, T> {}

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
