use std::{
    any::{Any, TypeId, type_name},
    cell::UnsafeCell,
    hash::{BuildHasher, Hash},
};

use fxhash::{FxBuildHasher, FxHashMap};
use indexmap::{IndexMap, IndexSet};

use crate::{commands::bundle::ComponentBundle, ecs::Component, entity::Entity};

#[cfg(feature = "reactivity")]
use crate::reactivity::TRACKED_COMPONENTS;

pub(crate) trait AnyColumn: Any {
    unsafe fn swap_remove_erased(&mut self, idx: usize);
    fn as_any_mut(&mut self) -> &mut dyn Any;
    unsafe fn move_row_erased(&mut self, index: usize, dst: *mut dyn AnyColumn);
    fn clone_empty(&self) -> Box<dyn AnyColumn>;
}

impl<T: Component> AnyColumn for Vec<T> {
    unsafe fn swap_remove_erased(&mut self, idx: usize) {
        self.swap_remove(idx);
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    unsafe fn move_row_erased(&mut self, index: usize, dst: *mut dyn AnyColumn) {
        let item = self.swap_remove(index);

        let dst_vec = unsafe {
            (&mut *dst)
                .as_any_mut()
                .downcast_mut::<Vec<T>>()
                .expect("Archetype component column type mismatch")
        };

        dst_vec.push(item);
    }

    fn clone_empty(&self) -> Box<dyn AnyColumn> {
        Box::new(Vec::<T>::new())
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash)]
pub(crate) struct ArchetypeId(u32);

impl ArchetypeId {
    pub(crate) fn new(id: u32) -> ArchetypeId {
        ArchetypeId(id)
    }
    pub(crate) fn id(&self) -> u32 {
        self.0
    }
}

pub struct ComponentColumn {
    pub(crate) data: Box<dyn AnyColumn>,
}

pub struct Archetype {
    pub(crate) id: ArchetypeId,
    pub(crate) types: IndexSet<TypeId, FxBuildHasher>,
    pub(crate) entities: Vec<Entity>,
    pub(crate) columns: UnsafeCell<IndexMap<TypeId, ComponentColumn, FxBuildHasher>>,
    pub(crate) type_names: IndexSet<&'static str, FxBuildHasher>,
}

unsafe impl Sync for Archetype {}
unsafe impl Send for Archetype {}

impl Archetype {
    pub(crate) fn new(
        id: ArchetypeId,
        types: IndexSet<TypeId, FxBuildHasher>,
        columns: IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
        type_names: IndexSet<&'static str, FxBuildHasher>,
    ) -> Self {
        Self {
            id,
            types,
            entities: Vec::new(),
            columns: std::cell::UnsafeCell::new(columns),
            type_names,
        }
    }

    /// # Safety
    ///
    /// * **Aliasing**: The caller must guarantee that no other mutable or immutable references
    ///   to this specific column's `Vec<T>` (or its contents) exist simultaneously.
    /// * **Data Races**: This function returns a raw pointer. Accessing or mutating the underlying
    ///   vector across threads without explicit synchronization causes a data race.
    /// * **In**: Modifying the vector (e.g., pushing/popping) may trigger a reallocation,
    ///   immediately invalidating any previously derived pointers or references to its elements.
    pub unsafe fn fetch_column_raw<T: 'static>(&self) -> *mut Vec<T> {
        unsafe {
            let cols = &mut *self.columns.get();
            let col = cols
                .get_mut(&std::any::TypeId::of::<T>())
                .unwrap_or_else(|| panic!("Column missing of: {:?}", type_name::<T>()));
            col.data
                .as_any_mut()
                .downcast_mut::<Vec<T>>()
                .expect("Type mismatch!") as *mut Vec<T>
        }
    }

    /// # Safety
    ///
    /// * **Aliasing**: The caller must guarantee that no other mutable or immutable references
    ///   to this specific column's `Vec<T>` (or its contents) exist simultaneously.
    /// * **Data Races**: This function returns a raw pointer. Accessing or mutating the underlying
    ///   vector across threads without explicit synchronization causes a data race.
    /// * **In**: Modifying the vector (e.g., pushing/popping) may trigger a reallocation,
    ///   immediately invalidating any previously derived pointers or references to its elements.
    pub unsafe fn fetch_column_raw_opt<T: 'static>(&self) -> Option<*mut Vec<T>> {
        let cols = unsafe { &mut *self.columns.get() };
        let col = cols.get_mut(&TypeId::of::<T>())?;
        let vec_ptr = col.data.as_any_mut().downcast_mut::<Vec<T>>()? as *mut Vec<T>;
        Some(vec_ptr)
    }

    pub(crate) fn id(&self) -> u32 {
        self.id.0
    }
}

pub(crate) struct ArchetypeManager {
    index: FxHashMap<u64, ArchetypeId>,
    pub(crate) archetypes: FxHashMap<ArchetypeId, Archetype>,
    pub(crate) next_id: u32,
}

impl ArchetypeManager {
    pub(crate) fn new() -> Self {
        Self {
            index: FxHashMap::default(),
            archetypes: FxHashMap::default(),
            next_id: 0,
        }
    }

    fn calculate_hash(&self, types: &IndexSet<TypeId, FxBuildHasher>) -> u64 {
        let mut combined_hash: u64 = 0;
        let hasher_builder = self.index.hasher();
        for type_id in types {
            combined_hash = combined_hash.wrapping_add(hasher_builder.hash_one(type_id));
        }
        combined_hash
    }

    #[cfg(feature = "reactivity")]
    pub(crate) fn sync_tracking_markers(&self, types: &mut IndexSet<TypeId, FxBuildHasher>) {
        let tracked = TRACKED_COMPONENTS.read();
        for meta in tracked.values() {
            if types.contains(&meta.component_id) {
                types.insert(meta.marker_id);
            } else {
                types.swap_remove(&meta.marker_id);
            }
        }
    }

    pub(crate) fn find_target_id_for_addition(
        &self,
        old_id: ArchetypeId,
        incoming_ids: &[TypeId],
    ) -> Option<ArchetypeId> {
        let old_arch = self.archetypes.get(&old_id)?;
        let mut target_types = old_arch.types.clone();
        for id in incoming_ids {
            target_types.insert(*id);
        }

        #[cfg(feature = "reactivity")]
        self.sync_tracking_markers(&mut target_types);

        let hash = self.calculate_hash(&target_types);
        self.index.get(&hash).copied()
    }

    pub(crate) fn find_target_id_for_subtraction(
        &self,
        old_id: ArchetypeId,
        removed_ids: &[TypeId],
    ) -> Option<ArchetypeId> {
        let old_arch = self.archetypes.get(&old_id)?;
        let mut target_types = old_arch.types.clone();
        for id in removed_ids {
            target_types.swap_remove(id);
        }

        #[cfg(feature = "reactivity")]
        self.sync_tracking_markers(&mut target_types);

        let hash = self.calculate_hash(&target_types);
        self.index.get(&hash).copied()
    }

    pub(crate) fn get_or_create_from_set(
        &mut self,
        types_set: IndexSet<TypeId, FxBuildHasher>,
        types_names_set: IndexSet<&'static str, FxBuildHasher>,
    ) -> ArchetypeId {
        let order_independent_hash = self.calculate_hash(&types_set);
        if let Some(id) = self.index.get(&order_independent_hash).copied() {
            return id;
        }

        let new_id = ArchetypeId::new(self.next_id);
        self.next_id += 1;

        let columns = IndexMap::with_hasher(FxBuildHasher::default());
        let new_arch = Archetype::new(new_id, types_set, columns, types_names_set);

        self.index.insert(order_independent_hash, new_id);
        self.archetypes.insert(new_id, new_arch);
        new_id
    }

    pub(crate) fn get_or_create_from_generic<T: ComponentBundle>(&mut self) -> ArchetypeId {
        let incoming_ids = T::get_type_ids();
        let mut types_set =
            IndexSet::with_capacity_and_hasher(incoming_ids.len(), FxBuildHasher::default());
        for &id in incoming_ids {
            types_set.insert(id);
        }

        #[cfg(feature = "reactivity")]
        self.sync_tracking_markers(&mut types_set);

        let order_independent_hash = self.calculate_hash(&types_set);
        if let Some(id) = self.index.get(&order_independent_hash).copied() {
            return id;
        }

        let incoming_names = T::get_type_names();
        let names_ref = incoming_names.as_ref();
        let mut types_names_set =
            IndexSet::with_capacity_and_hasher(names_ref.len(), FxBuildHasher::default());
        for &name in names_ref {
            types_names_set.insert(name);
        }

        let new_id = ArchetypeId(self.next_id);
        self.next_id += 1;

        let mut columns = IndexMap::with_hasher(FxBuildHasher::default());
        T::create_empty_columns(&mut columns);

        #[cfg(feature = "reactivity")]
        {
            let tracked = TRACKED_COMPONENTS.read();
            tracked
                .values()
                .filter(|m| types_set.contains(&m.marker_id))
                .for_each(|m| {
                    columns.insert(m.marker_id, (m.create_marker_column)());
                });
        }
        let new_arch = Archetype::new(new_id, types_set, columns, types_names_set);
        self.index.insert(order_independent_hash, new_id);
        self.archetypes.insert(new_id, new_arch);
        new_id
    }

    pub(crate) fn get(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.archetypes.get(&id)
    }

    pub(crate) fn get_mut(&mut self, id: ArchetypeId) -> Option<&mut Archetype> {
        self.archetypes.get_mut(&id)
    }
}
