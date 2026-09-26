use std::{any::TypeId, marker::PhantomData};

use crate::{ecs::Component, system::AccessVec, world::archetypes::Archetype};

pub trait QueryFilter {
    type FilterData: Send + Sync;
    fn init_filter_data(archetype: &Archetype) -> Self::FilterData;
    fn matches(archetype: &Archetype) -> bool;
    fn matches_row(filter_data: &Self::FilterData, row_idx: usize) -> bool;
    fn collect_filter(withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>);
}

#[derive(Debug)]
pub struct With<T: Component>(PhantomData<T>);
impl<T: Component> QueryFilter for With<T> {
    type FilterData = bool;
    fn init_filter_data(archetype: &Archetype) -> Self::FilterData {
        archetype.has_column::<T>()
    }
    fn matches(archetype: &Archetype) -> bool {
        archetype.has_column::<T>()
    }
    fn matches_row(filter_data: &Self::FilterData, _row_idx: usize) -> bool {
        *filter_data
    }
    fn collect_filter(withs: &mut AccessVec<TypeId>, _withouts: &mut AccessVec<TypeId>) {
        withs.push(std::any::TypeId::of::<T>());
    }
}

#[derive(Debug)]
pub struct Without<T: Component>(PhantomData<T>);
impl<T: Component> QueryFilter for Without<T> {
    type FilterData = bool;
    fn init_filter_data(archetype: &Archetype) -> Self::FilterData {
        !archetype.has_column::<T>()
    }
    fn matches(archetype: &Archetype) -> bool {
        !archetype.has_column::<T>()
    }
    fn matches_row(filter_data: &Self::FilterData, _row_idx: usize) -> bool {
        *filter_data
    }
    fn collect_filter(_withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>) {
        withouts.push(std::any::TypeId::of::<T>());
    }
}

pub struct Or<T>(PhantomData<T>);

pub struct Not<F>(PhantomData<F>);

impl<F: QueryFilter> QueryFilter for Not<F> {
    type FilterData = (Option<F::FilterData>, bool);

    #[inline]
    fn init_filter_data(archetype: &Archetype) -> Self::FilterData {
        if F::matches(archetype) {
            (Some(F::init_filter_data(archetype)), true)
        } else {
            (None, false)
        }
    }

    #[inline]
    fn matches(_archetype: &Archetype) -> bool {
        true
    }

    #[inline]
    fn matches_row(filter_data: &Self::FilterData, row_idx: usize) -> bool {
        if filter_data.1 {
            !F::matches_row(filter_data.0.as_ref().unwrap(), row_idx)
        } else {
            true
        }
    }

    #[inline]
    fn collect_filter(withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>) {
        F::collect_filter(withouts, withs);
    }
}

macro_rules! impl_or_tuple {
    ($($name:ident),*) => {
        impl<$($name: QueryFilter),*> QueryFilter for Or<($($name,)*)> {
            type FilterData = ($( Option<$name::FilterData>, )*);

            #[inline]
            fn init_filter_data(archetype: &Archetype) -> Self::FilterData {
                ($(
                    if $name::matches(archetype) {
                        Some($name::init_filter_data(archetype))
                    } else {
                        None
                    },
                )*)
            }

            #[inline]
            fn matches(_archetype: &Archetype) -> bool {
                true
            }

            #[inline]
            fn matches_row(filter_data: &Self::FilterData, row_idx: usize) -> bool {
                #[allow(non_snake_case)]
                let ($($name,)*) = filter_data;
                $(
                    $name.as_ref().map_or(false, |inner_data| $name::matches_row(inner_data, row_idx))
                )||*
            }

            #[inline]
            fn collect_filter(withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>) {
                $(
                    $name::collect_filter(withs, withouts);
                )*
            }
        }
    };
}

impl_or_tuple!(A, B);
impl_or_tuple!(A, B, C);
impl_or_tuple!(A, B, C, D);
impl_or_tuple!(A, B, C, D, E);
impl_or_tuple!(A, B, C, D, E, F);
impl_or_tuple!(A, B, C, D, E, F, G);
impl_or_tuple!(A, B, C, D, E, F, G, H);
impl_or_tuple!(A, B, C, D, E, F, G, H, I);
impl_or_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_or_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_or_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

#[derive(Debug)]
pub struct EmptyQueryFilter;
impl QueryFilter for EmptyQueryFilter {
    type FilterData = ();
    fn init_filter_data(_archetype: &Archetype) -> Self::FilterData {}
    fn matches(_archetype: &Archetype) -> bool {
        true
    }
    fn matches_row(_filter_data: &Self::FilterData, _row_idx: usize) -> bool {
        true
    }
    fn collect_filter(_withs: &mut AccessVec<TypeId>, _withouts: &mut AccessVec<TypeId>) {}
}

macro_rules! impl_query_filter_tuple {
    ($($name:ident),*) => {
        impl<$($name: QueryFilter),*> QueryFilter for ($($name,)*) {
            type FilterData = ($($name::FilterData,)*);
            #[inline]
            fn init_filter_data(archetype: &Archetype) -> Self::FilterData {
                ($($name::init_filter_data(archetype),)*)
            }
            #[inline]
            fn matches(archetype: &Archetype) -> bool {
                $($name::matches(archetype))&&*
            }
            #[inline]
            fn matches_row(filter_data: &Self::FilterData, row_idx: usize) -> bool {
                #[allow(non_snake_case)]
                let ($($name,)*) = filter_data;
                $($name::matches_row($name, row_idx))&&*
            }
            #[inline]
            fn collect_filter(withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>) {
                $(
                    $name::collect_filter(withs, withouts);
                )*
            }
        }
    };
}

impl_query_filter_tuple!(A);
impl_query_filter_tuple!(A, B);
impl_query_filter_tuple!(A, B, C);
impl_query_filter_tuple!(A, B, C, D);
impl_query_filter_tuple!(A, B, C, D, E);
impl_query_filter_tuple!(A, B, C, D, E, F);
impl_query_filter_tuple!(A, B, C, D, E, F, G);
impl_query_filter_tuple!(A, B, C, D, E, F, G, H);
impl_query_filter_tuple!(A, B, C, D, E, F, G, H, I);
impl_query_filter_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_query_filter_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_query_filter_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_query_filter_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
