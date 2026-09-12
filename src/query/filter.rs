use std::{any::TypeId, marker::PhantomData};

use crate::{
    ecs::Component,
    system::{AccessHashSet, AccessVec},
    world::archetypes::Archetype,
};

pub trait StructuralQueryFilter: QueryFilter {}

impl<T: Component> StructuralQueryFilter for With<T> {}
impl<T: Component> StructuralQueryFilter for Without<T> {}
impl<T: QueryFilter + StructuralQueryFilter> StructuralQueryFilter for Or<T> where Or<T>: QueryFilter
{}
impl<T: QueryFilter + StructuralQueryFilter> StructuralQueryFilter for Not<T> {}

pub trait QueryFilter {
    fn matches(types: &AccessHashSet<TypeId>) -> bool;
    fn matches_negated(types: &AccessHashSet<TypeId>) -> bool {
        !Self::matches(types)
    }
    fn collect_filter(
        withs: &mut AccessVec<std::any::TypeId>,
        withouts: &mut AccessVec<std::any::TypeId>,
    );
    fn filter_indices(_archetype: &Archetype, _indices: &mut Vec<usize>) {}
}

#[derive(Debug)]
pub struct With<T: Component>(PhantomData<T>);
impl<T: Component> QueryFilter for With<T> {
    fn matches(types: &AccessHashSet<TypeId>) -> bool {
        types.contains(&TypeId::of::<T>())
    }
    fn collect_filter(withs: &mut AccessVec<TypeId>, _withouts: &mut AccessVec<TypeId>) {
        withs.push(std::any::TypeId::of::<T>());
    }
}

#[derive(Debug)]
pub struct Without<T: Component>(PhantomData<T>);
impl<T: Component> QueryFilter for Without<T> {
    fn matches(types: &AccessHashSet<TypeId>) -> bool {
        !types.contains(&TypeId::of::<T>())
    }
    fn collect_filter(_withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>) {
        withouts.push(std::any::TypeId::of::<T>());
    }
}

pub struct Or<T>(PhantomData<T>);

pub struct Not<F>(PhantomData<F>);

impl<F: QueryFilter + StructuralQueryFilter> QueryFilter for Not<F> {
    #[inline]
    fn matches(types: &AccessHashSet<TypeId>) -> bool {
        F::matches_negated(types)
    }

    #[inline]
    fn collect_filter(withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>) {
        F::collect_filter(withouts, withs);
    }
}

macro_rules! impl_or_tuple {
    ($($name:ident),*) => {
        impl<$($name: QueryFilter + StructuralQueryFilter),*> QueryFilter for Or<($($name,)*)> {
            #[inline]
            fn matches(types: &AccessHashSet<TypeId>) -> bool {
                $($name::matches(types))||*
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

impl_or_tuple!(A);
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
    fn matches(_: &AccessHashSet<TypeId>) -> bool {
        true
    }
    fn collect_filter(_withs: &mut AccessVec<TypeId>, _withouts: &mut AccessVec<TypeId>) {}
}

macro_rules! impl_query_filter_tuple {
    ($($name:ident),*) => {
        impl<$($name: QueryFilter),*> QueryFilter for ($($name,)*) {
            #[inline]
            fn matches(types: &AccessHashSet<TypeId>) -> bool {
                $($name::matches(types))&&*
            }

            fn matches_negated(types: &AccessHashSet<TypeId>) -> bool {
                $($name::matches_negated(types))&&*
            }

            #[inline]
            fn collect_filter(withs: &mut AccessVec<TypeId>, withouts: &mut AccessVec<TypeId>) {
                $(
                    $name::collect_filter(withs, withouts);
                )*
            }

            #[inline]
            fn filter_indices(archetype: &Archetype, indices: &mut Vec<usize>) {
                $(
                    $name::filter_indices(archetype, indices);
                )*
            }
        }
        impl<$($name: QueryFilter + StructuralQueryFilter),*> StructuralQueryFilter for ($($name,)*){}
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
