use fxhash::FxBuildHasher;
use indexmap::IndexSet;
use std::{any::TypeId, marker::PhantomData};
use crate::ecs::Component;
#[cfg(feature = "reactivity")]
use crate::{
    reactivity::{ChangedMarker, Mut},
    world::storage::CurrentBufferIdx,
};
use crate::{entity::Entity, query::QueryData, system::AccessVec, world::archetypes::Archetype};

impl<T: Component> QueryData for &T {
    type Item<'w> = &'w T;
    type ReadOnlyItem<'w> = &'w T;
    type Fetch = *const T;

    fn matches(types: &IndexSet<TypeId, FxBuildHasher>) -> bool {
        types.contains(&TypeId::of::<T>())
    }
    fn collect_access(
        reads: &mut AccessVec<std::any::TypeId>,
        _writes: &mut AccessVec<std::any::TypeId>,
    ) {
        reads.push(TypeId::of::<T>());
    }
    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        unsafe { (*archetype.fetch_column_raw::<T>()).as_ptr() }
    }
    unsafe fn fetch_mut<'w>(fetch: Self::Fetch, index: usize) -> Self::Item<'w> {
        unsafe { &*fetch.add(index) }
    }
    unsafe fn fetch_read_only<'w>(fetch: Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        unsafe { &*fetch.add(index) }
    }
}

#[cfg(not(feature = "reactivity"))]
impl<T: Component> QueryData for &mut T {
    type Item<'w> = &'w mut T;
    type ReadOnlyItem<'w> = &'w T;
    type Fetch = *mut T;

    fn matches(types: &IndexSet<TypeId, FxBuildHasher>) -> bool {
        types.contains(&TypeId::of::<T>())
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        unsafe { (*archetype.fetch_column_raw::<T>()).as_mut_ptr() }
    }

    #[inline(always)]
    unsafe fn fetch_mut<'w>(fetch: Self::Fetch, index: usize) -> Self::Item<'w> {
        unsafe { &mut *fetch.add(index) }
    }

    unsafe fn fetch_read_only<'w>(fetch: Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        unsafe { &*fetch.add(index) }
    }

    fn collect_access(_reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
        writes.push(TypeId::of::<T>());
    }
}

#[cfg(feature = "reactivity")]
impl<T: Component> QueryData for &mut T {
    type Item<'w> = Mut<'w, T>;
    type ReadOnlyItem<'w> = &'w T;
    type Fetch = (*mut T, *mut ChangedMarker<T>, u8, bool);

    fn matches(types: &IndexSet<TypeId, FxBuildHasher>) -> bool {
        types.contains(&TypeId::of::<T>())
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        unsafe {
            let data_ptr = (*archetype.fetch_column_raw::<T>()).as_mut_ptr();
            let columns = &mut *archetype.columns.get();
            let marker_id = TypeId::of::<ChangedMarker<T>>();
            let current_write_idx = CurrentBufferIdx::current_write_idx();

            if let Some(column) = columns.get_mut(&marker_id) {
                let vec_ptr = column
                    .data
                    .as_any_mut()
                    .downcast_mut::<Vec<ChangedMarker<T>>>()
                    .unwrap();

                (data_ptr, vec_ptr.as_mut_ptr(), current_write_idx, true)
            } else {
                (data_ptr, std::ptr::null_mut(), current_write_idx, false)
            }
        }
    }

    #[inline(always)]
    unsafe fn fetch_mut<'w>(fetch: Self::Fetch, index: usize) -> Self::Item<'w> {
        unsafe {
            Mut {
                value: fetch.0.add(index),
                marker: fetch.1.wrapping_add(index),
                current_write_idx: fetch.2,
                should_modify: fetch.3,
                _marker: std::marker::PhantomData,
            }
        }
    }

    unsafe fn fetch_read_only<'w>(fetch: Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        unsafe { &*fetch.0.add(index) }
    }

    fn collect_access(_reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
        writes.push(TypeId::of::<T>());
        writes.push(TypeId::of::<ChangedMarker<T>>());
    }
}

impl QueryData for Entity {
    type Item<'w> = &'w Entity;
    type ReadOnlyItem<'w> = &'w Entity;
    type Fetch = *const Entity;

    fn matches(_types: &IndexSet<TypeId, FxBuildHasher>) -> bool {
        true
    }
    fn collect_access(_reads: &mut AccessVec<TypeId>, _writes: &mut AccessVec<TypeId>) {}
    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        archetype.entities.as_ptr()
    }
    unsafe fn fetch_mut<'w>(fetch: Self::Fetch, index: usize) -> Self::Item<'w> {
        unsafe { &*fetch.add(index) }
    }
    unsafe fn fetch_read_only<'w>(fetch: Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        unsafe { &*fetch.add(index) }
    }
}

impl<T: Component> QueryData for Option<&T> {
    type Item<'w> = Option<&'w T>;
    type ReadOnlyItem<'w> = Option<&'w T>;
    type Fetch = Option<*const T>;

    fn matches(_types: &IndexSet<TypeId, FxBuildHasher>) -> bool {
        true
    }

    fn collect_access(reads: &mut AccessVec<TypeId>, _writes: &mut AccessVec<TypeId>) {
        reads.push(TypeId::of::<T>());
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        if archetype.types.contains(&TypeId::of::<T>()) {
            unsafe { Some((*archetype.fetch_column_raw::<T>()).as_mut_ptr()) }
        } else {
            None
        }
    }
    unsafe fn fetch_mut<'w>(fetch: Self::Fetch, index: usize) -> Self::Item<'w> {
        if let Some(fetch) = fetch {
            unsafe { Some(&*fetch.add(index)) }
        } else {
            None
        }
    }

    unsafe fn fetch_read_only<'w>(fetch: Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        if let Some(fetch) = fetch {
            unsafe { Some(&*fetch.add(index)) }
        } else {
            None
        }
    }
}

#[cfg(not(feature = "reactivity"))]
impl<T: Component> QueryData for Option<&mut T> {
    type Item<'w> = Option<&'w mut T>;
    type ReadOnlyItem<'w> = Option<&'w T>;
    type Fetch = Option<*mut T>;

    fn matches(_types: &IndexSet<TypeId, FxBuildHasher>) -> bool {
        true
    }

    fn collect_access(_reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
        writes.push(TypeId::of::<T>());
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        unsafe {
            let columns = &mut *archetype.columns.get();
            let component_id = TypeId::of::<T>();
            if columns.contains_key(&component_id) {
                let data_ptr = (*archetype.fetch_column_raw::<T>()).as_mut_ptr();
                Some(data_ptr)
            } else {
                None
            }
        }
    }

    #[inline(always)]
    unsafe fn fetch_mut<'w>(fetch: Self::Fetch, index: usize) -> Self::Item<'w> {
        unsafe {
            if let Some(data_head) = fetch {
                Some(&mut *data_head.add(index))
            } else {
                None
            }
        }
    }

    #[inline(always)]
    unsafe fn fetch_read_only<'w>(fetch: Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        unsafe {
            if let Some(data_head) = fetch {
                Some(&*data_head.add(index))
            } else {
                None
            }
        }
    }
}

#[cfg(feature = "reactivity")]

impl<T: Component> QueryData for Option<&mut T> {
    type Item<'w> = Option<Mut<'w, T>>;
    type ReadOnlyItem<'w> = Option<&'w T>;
    type Fetch = Option<(*mut T, *mut ChangedMarker<T>, u8, bool)>;

    fn matches(_types: &IndexSet<TypeId, FxBuildHasher>) -> bool {
        true
    }

    fn collect_access(_reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
        writes.push(TypeId::of::<T>());
        writes.push(TypeId::of::<ChangedMarker<T>>());
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        unsafe {
            let columns = &mut *archetype.columns.get();
            let component_id = TypeId::of::<T>();
            let marker_id = TypeId::of::<ChangedMarker<T>>();
            let current_write_idx = CurrentBufferIdx::current_write_idx();
            if columns.contains_key(&component_id) {
                let data_ptr = (*archetype.fetch_column_raw::<T>()).as_mut_ptr();

                if let Some(column) = columns.get_mut(&marker_id) {
                    let vec_ptr = column
                        .data
                        .as_any_mut()
                        .downcast_mut::<Vec<ChangedMarker<T>>>()
                        .unwrap();

                    Some((data_ptr, vec_ptr.as_mut_ptr(), current_write_idx, true))
                } else {
                    Some((data_ptr, std::ptr::null_mut(), current_write_idx, false))
                }
            } else {
                None
            }
        }
    }

    #[inline(always)]
    unsafe fn fetch_mut<'w>(fetch: Self::Fetch, index: usize) -> Self::Item<'w> {
        unsafe {
            if let Some(data) = fetch {
                Some(Mut {
                    value: data.0.add(index),
                    marker: data.1.add(index),
                    current_write_idx: data.2,
                    should_modify: data.3,
                    _marker: std::marker::PhantomData,
                })
            } else {
                None
            }
        }
    }

    #[inline(always)]
    unsafe fn fetch_read_only<'w>(fetch: Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
        unsafe {
            if let Some(data_head) = fetch {
                Some(&*data_head.0.add(index))
            } else {
                None
            }
        }
    }
}

pub struct Has<T: Component>(PhantomData<T>);

impl<T: Component> QueryData for Has<T> {
    type Item<'w> = bool;
    type ReadOnlyItem<'w> = bool;
    type Fetch = bool;
    fn collect_access(
        _reads: &mut AccessVec<std::any::TypeId>,
        _writes: &mut AccessVec<std::any::TypeId>,
    ) {
    }

    fn matches(types: &IndexSet<TypeId, FxBuildHasher>) -> bool {
        types.contains(&TypeId::of::<T>())
    }

    unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch {
        unsafe { archetype.fetch_column_raw_opt::<T>().is_some() }
    }

    unsafe fn fetch_mut<'w>(fetch: Self::Fetch, _index: usize) -> Self::Item<'w> {
        fetch
    }

    unsafe fn fetch_read_only<'w>(fetch: Self::Fetch, _index: usize) -> Self::ReadOnlyItem<'w> {
        fetch
    }
}

macro_rules! impl_world_query_tuple {
    ($($name:ident -> $idx:tt),*) => {
        impl<$($name: QueryData),*> QueryData for ($($name,)*) {
            type Item<'w> = ($($name::Item<'w>,)*);
            type ReadOnlyItem<'w> = ($($name::ReadOnlyItem<'w>,)*);
            type Fetch = ($($name::Fetch,)*);

            fn matches(types: &IndexSet<TypeId, FxBuildHasher>) -> bool { $($name::matches(types))&&* }
            unsafe fn init_fetch(archetype: &Archetype) -> Self::Fetch { unsafe { ($($name::init_fetch(archetype),)*) } }
            unsafe fn fetch_mut<'w>(fetch: Self::Fetch, index: usize) -> Self::Item<'w> {
                unsafe {($($name::fetch_mut(fetch.$idx, index),)*)}
            }

            unsafe fn fetch_read_only<'w>(fetch: Self::Fetch, index: usize) -> Self::ReadOnlyItem<'w> {
                unsafe { ($($name::fetch_read_only(fetch.$idx, index),)*) }
            }

            fn collect_access(reads: &mut AccessVec<TypeId>, writes: &mut AccessVec<TypeId>) {
                $( $name::collect_access(reads, writes); )*
            }
        }
    };
}

impl_world_query_tuple!(A -> 0);
impl_world_query_tuple!(A -> 0, B -> 1);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5, G -> 6);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5, G -> 6, H -> 7);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5, G -> 6, H -> 7, I -> 8);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5, G -> 6, H -> 7, I -> 8, J -> 9);
impl_world_query_tuple!(A -> 0, B -> 1, C -> 2, D -> 3, E -> 4, F -> 5, G -> 6, H -> 7, I -> 8, J -> 9, K -> 10);
