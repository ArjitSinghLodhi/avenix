use parking_lot::RwLock;
use std::{any::TypeId, marker::PhantomData};

use crate::{
    ecs::Component,
    extensions::{Archetype, ComponentColumn, ComponentColumnRead},
    query::{QueryData, QueryFilter},
    reactivity::{TRACKED_COMPONENTS, TrackedComponentMeta, verify_app_not_build_yet},
};

pub(crate) fn register_added_tracked_component<T: Component>() {
    let _lock = verify_app_not_build_yet();
    let mut tracked = TRACKED_COMPONENTS.lock();
    let component_id = TypeId::of::<T>();
    let marker_id = TypeId::of::<AddedMarker<T>>();

    let meta_list = tracked.entry(component_id).or_default();

    if meta_list.iter().any(|m| m.marker_id == marker_id) {
        return;
    }

    meta_list.push(TrackedComponentMeta {
        marker_id,
        create_marker_column: || ComponentColumn {
            data: RwLock::new(Box::new(Vec::<AddedMarker<T>>::new())),
        },
        push_default_marker: |column| {
            let mut guard = column.data.write();
            let raw_any = guard.as_any_mut();
            let vec = raw_any.downcast_mut::<Vec<AddedMarker<T>>>().unwrap();
            vec.push(AddedMarker {
                read: false,
                write: true,
                phantom: PhantomData,
            });
        },
        clear_column_markers: |raw_any| {
            let vec = raw_any.downcast_mut::<Vec<AddedMarker<T>>>().unwrap();
            vec.iter_mut().for_each(|marker| {
                marker.read = marker.write;
                marker.write = false;
            });
        },
    });
}

pub struct AddedMarker<T> {
    read: bool,
    write: bool,
    phantom: PhantomData<T>,
}

impl<T: Component> Component for AddedMarker<T> {}

/// A query filter that matches components of type `T` that were newly added during the previous frame.
///
/// # Architecture & Timing
///
/// Component addition tracking in Avenix is globally double-buffered and strictly time-bound.
/// It operates on a strict 3-frame lifecycle:
///
/// * **Frame 1 (Addition):** The component is structurally added or spawned. The tracking bool is set internally but is **not** yet visible to queries.
/// * **Frame 2 (Detection Window):** The addition becomes globally visible. Any system running in this frame filtering for `Added<T>` will detect it.
/// * **Frame 3 (Purge):** The addition state is unconditionally cleared and resets to `false`.
///
/// Regardless of whether a system ran or read the data, the detection flag will never last for more than exactly one frame.
pub struct Added<T: Component>(std::marker::PhantomData<T>);

impl<T: Component> QueryFilter for Added<T> {
    type FilterData<'w> = ComponentColumnRead<'w, AddedMarker<T>>;
    fn init_filter_data<'w>(archetype: &Archetype) -> Self::FilterData<'w> {
        archetype.get_column::<AddedMarker<T>>()
    }
    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }
    fn matches_row<'w>(filter_data: &Self::FilterData<'w>, row_idx: usize) -> bool {
        unsafe { (*filter_data.as_ptr().add(row_idx)).read }
    }
    fn collect_filter(
        withs: &mut crate::extensions::AccessVec<std::any::TypeId>,
        _withouts: &mut crate::extensions::AccessVec<std::any::TypeId>,
    ) {
        withs.push(TypeId::of::<AddedMarker<T>>());
        register_added_tracked_component::<T>();
    }
}

/// A query data wrapper that allows systems to inspect the addition state of an individual component instance.
///
/// Unlike the [`Added<T>`] filter, which excludes non-added entities from the query entirely,
/// `AddedTracker<T>` allows the entity to match the query while exposing the [`.is_added()`] method
/// to check its status conditionally.
///
/// # Architecture & Timing
///
/// This tracking relies on the exact same double-buffered, frame-locked system as the filter:
///
/// * **Frame 1:** The target component is added or spawned. The tracking bool is set but remains hidden.
/// * **Frame 2:** [`.is_added()`] evaluates to `true` globally across this entire frame window.
/// * **Frame 3:** The flag is automatically purged and resets to `false`, regardless of system execution.
///
/// [`.is_added()`]: AddedTracker::is_added
pub struct AddedTracker<T: Component> {
    added: bool,
    _phantom: PhantomData<T>,
}

impl<T: Component> AddedTracker<T> {
    /// Returns whether the target component was newly added to the entity during the previous frame.
    #[inline(always)]
    pub fn is_added(&self) -> bool {
        self.added
    }
}

impl<T: Component> QueryData for AddedTracker<T> {
    type Item<'w> = AddedTracker<T>;
    type ReadOnlyItem<'w> = AddedTracker<T>;
    type Fetch<'w> = ComponentColumnRead<'w, AddedMarker<T>>;
    fn collect_access(
        reads: &mut crate::extensions::AccessVec<std::any::TypeId>,
        _writes: &mut crate::extensions::AccessVec<std::any::TypeId>,
    ) {
        reads.push(TypeId::of::<AddedMarker<T>>());
        register_added_tracked_component::<T>();
    }
    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }
    unsafe fn init_fetch<'w>(archetype: &'w Archetype) -> Self::Fetch<'w> {
        archetype.get_column::<AddedMarker<T>>()
    }

    unsafe fn fetch_mut<'w, 'b>(fetch: &Self::Fetch<'b>, index: usize) -> Self::Item<'w> {
        let marker_ref = unsafe { &*fetch.as_ptr().add(index) };
        let added = marker_ref.read;
        AddedTracker {
            added,
            _phantom: PhantomData,
        }
    }

    unsafe fn fetch_read_only<'w, 'b>(
        fetch: &Self::Fetch<'b>,
        index: usize,
    ) -> Self::ReadOnlyItem<'w> {
        let marker_ref = unsafe { &(*fetch.as_ptr().add(index)) };
        let added = marker_ref.read;
        AddedTracker {
            added,
            _phantom: PhantomData,
        }
    }
}
