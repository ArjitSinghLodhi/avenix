use indexmap::IndexMap;
use parking_lot::RwLock;
use rustc_hash::FxBuildHasher;
use std::any::{TypeId, type_name};

use crate::{
    ecs::Component,
    world::archetypes::{Archetype, ComponentColumn},
};

pub trait ComponentBundle: Send + Sync + 'static {
    const TYPE_IDS: &[TypeId];
    fn get_type_ids() -> &'static [TypeId];
    fn push_to_archetype(self, archetype: &mut Archetype);

    /// # Safety
    ///
    /// * **Structural Alignment**: The caller must guarantee that the destination `archetype`
    ///   is properly configured and contains the exact matching type columns required by this bundle.
    /// * **Bounds Allocation**: The `row_idx` must be within the bounds of the allocated
    ///   component arrays, or exactly equal to the current length to append data.
    /// * **Memory Lifecycle**: If an active component already occupies `row_idx`, the implementation
    ///   must safely run its destructor (`ptr::drop_in_place`) before overwriting it with `ptr::write`
    ///   to prevent memory leaks.
    unsafe fn insert_to_archetype(self, archetype: &mut Archetype, row_idx: usize);
    fn create_empty_columns(columns: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>);
    type NamesArray: AsRef<[&'static str]>;
    fn get_type_names() -> Self::NamesArray;
}

impl<T: Component> ComponentBundle for T {
    const TYPE_IDS: &[TypeId] = &[TypeId::of::<T>()];
    fn get_type_ids() -> &'static [TypeId] {
        Self::TYPE_IDS
    }
    fn create_empty_columns(columns: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>) {
        let id = TypeId::of::<T>();
        columns.insert(
            id,
            ComponentColumn {
                data: RwLock::new(Box::new(Vec::<T>::new())),
            },
        );
    }
    fn push_to_archetype(self, archetype: &mut Archetype) {
        let mut vec_gaurd = archetype.get_column_mut::<T>();
        vec_gaurd.push(self);
    }
    unsafe fn insert_to_archetype(self, archetype: &mut Archetype, row_idx: usize) {
        unsafe {
            let mut vec_gaurd = archetype.get_column_mut::<T>();
            if row_idx < vec_gaurd.len() {
                std::ptr::drop_in_place(&mut vec_gaurd[row_idx]);
                std::ptr::write(&mut vec_gaurd[row_idx], self);
            } else {
                vec_gaurd.push(self);
            }
        }
    }
    type NamesArray = [&'static str; 1];
    fn get_type_names() -> Self::NamesArray {
        [type_name::<T>()]
    }
}

macro_rules! impl_component_tuple {
    ($($val:ident : $T:ident),*) => {
        impl<$($T: Component),*> ComponentBundle for ($($T,)*) {

            const TYPE_IDS: &[TypeId] = &[ $( TypeId::of::<$T>() ),* ];

            fn get_type_ids() -> &'static [TypeId] {
                Self::TYPE_IDS
            }

            fn create_empty_columns(columns: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>) {
                $(
                    let id = TypeId::of::<$T>();
                    columns.insert(id, ComponentColumn {
                        data: RwLock::new(Box::new(Vec::<$T>::new())),
                    });
                )*
            }

            fn push_to_archetype(self, archetype: &mut Archetype) {
                #[allow(non_snake_case)]
                let ($($val,)*) = self;

                $(
                    #[allow(non_snake_case)]
                    let mut $T = archetype.get_column_mut::<$T>();
                )*

                $(
                    $T.push($val);
                )*
            }

            unsafe fn insert_to_archetype(self, archetype: &mut Archetype, row_idx: usize) {
                #[allow(non_snake_case)]
                let ($($val,)*) = self;

                $(
                    #[allow(non_snake_case)]
                    let mut $T = archetype.get_column_mut::<$T>();
                )*

                $(
                    if row_idx < $T.len() {
                        unsafe {
                            std::ptr::drop_in_place(&mut $T[row_idx]);
                            std::ptr::write(&mut $T[row_idx], $val);
                        }
                    } else {
                        $T.push($val);
                    }
                )*
            }

            type NamesArray = [&'static str; 0 $( + { let _ = stringify!($T); 1 } )*];

            #[inline(always)]
            fn get_type_names() -> Self::NamesArray {
                [ $( std::any::type_name::<$T>() ),* ]
            }
        }
    };
}

impl_component_tuple!(a: A);
impl_component_tuple!(a: A, b: B);
impl_component_tuple!(a: A, b: B, c: C);
impl_component_tuple!(a: A, b: B, c: C, d: D);
impl_component_tuple!(a: A, b: B, c: C, d: D, e: E);
impl_component_tuple!(a: A, b: B, c: C, d: D, e: E, f: F);
impl_component_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G);
impl_component_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H);
impl_component_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I);
impl_component_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J);
impl_component_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K);
impl_component_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K, l: L);
impl_component_tuple!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H, i: I, j: J, k: K, l: L, m: M);
