use std::{any::TypeId, marker::PhantomData};

use fxhash::FxBuildHasher;
use indexmap::IndexSet;

use crate::{
    ecs::Component,
    extensions::ComponentColumn,
    query::{QueryData, QueryFilter},
    reactivity::{TRACKED_COMPONENTS, TrackedComponentMeta},
    world::storage::CurrentBufferIdx,
};

pub(crate) fn register_added_tracked_component<T: Component>() {
    let mut tracked = TRACKED_COMPONENTS.write();
    tracked.insert(
        TypeId::of::<T>(),
        TrackedComponentMeta {
            component_id: TypeId::of::<T>(),
            marker_id: TypeId::of::<AddedMarker<T>>(),
            create_marker_column: || ComponentColumn {
                data: Box::new(Vec::<AddedMarker<T>>::new()),
            },
            push_default_marker: |column| {
                let raw_any = column.data.as_any_mut();
                let vec = raw_any.downcast_mut::<Vec<AddedMarker<T>>>().unwrap();
                let current_write_idx = CurrentBufferIdx::current_write_idx();
                let mut added_marker = [false; 2];
                added_marker[current_write_idx as usize] = true;
                vec.push(AddedMarker {
                    added_marker,
                    phantom: PhantomData,
                });
            },
            clear_column_markers: |raw_any| {
                let idx = CurrentBufferIdx::current_write_idx();
                let vec = raw_any.downcast_mut::<Vec<AddedMarker<T>>>().unwrap();
                vec.iter_mut().for_each(|marker| {
                    marker.added_marker[idx as usize] = false;
                });
            },
        },
    );
}

pub struct AddedMarker<T> {
    added_marker: [bool; 2],
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
    fn matches(types: &crate::extensions::AccessHashSet<TypeId>) -> bool {
        types.contains(&TypeId::of::<AddedMarker<T>>())
    }
    fn collect_filter(
        withs: &mut crate::extensions::AccessVec<std::any::TypeId>,
        _withouts: &mut crate::extensions::AccessVec<std::any::TypeId>,
    ) {
        withs.push(TypeId::of::<AddedMarker<T>>());
        register_added_tracked_component::<T>();
    }
    fn filter_indices(archetype: &crate::extensions::Archetype, indices: &mut Vec<usize>) {
        let marker_ptr = unsafe { (*archetype.fetch_column_raw::<AddedMarker<T>>()).as_ptr() };
        let current_read_idx = CurrentBufferIdx::current_read_idx();
        indices.retain(|idx| {
            unsafe { &*marker_ptr.add(*idx) }.added_marker[current_read_idx as usize]
        });
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
    type Fetch = (u8, *const AddedMarker<T>);
    fn collect_access(
        reads: &mut crate::extensions::AccessVec<std::any::TypeId>,
        _writes: &mut crate::extensions::AccessVec<std::any::TypeId>,
    ) {
        reads.push(TypeId::of::<AddedMarker<T>>());
        register_added_tracked_component::<T>();
    }
    fn matches(types: &IndexSet<TypeId, FxBuildHasher>) -> bool {
        types.contains(&TypeId::of::<AddedMarker<T>>())
    }
    unsafe fn init_fetch(archetype: &crate::extensions::Archetype) -> Self::Fetch {
        let marker_ptr = unsafe { (*archetype.fetch_column_raw::<AddedMarker<T>>()).as_ptr() };
        let current_read_idx = CurrentBufferIdx::current_read_idx();
        (current_read_idx, marker_ptr)
    }

    unsafe fn fetch_mut<'w>(fetch: Self::Fetch, index: usize) -> Self::Item<'w> {
        let marker_ref = unsafe { &*fetch.1.add(index) };
        let added = marker_ref.added_marker[fetch.0 as usize];
        AddedTracker {
            added,
            _phantom: PhantomData,
        }
    }

    unsafe fn fetch_read_only<'w>(fetch: Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        let marker_ref = unsafe { &(*fetch.1.add(index)) };
        let added = marker_ref.added_marker[fetch.0 as usize];
        AddedTracker {
            added,
            _phantom: PhantomData,
        }
    }
}
