use std::{
    any::{Any, TypeId, type_name},
    hash::{BuildHasher, Hash},
    mem::transmute,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use dashmap::{
    DashMap,
    mapref::one::{Ref, RefMut},
};
use indexmap::{IndexMap, IndexSet};
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::{commands::bundle::ComponentBundle, ecs::Component, entity::Entity};

#[cfg(feature = "reactivity")]
use crate::reactivity::TRACKED_COMPONENTS;

pub(crate) trait AnyColumn: Any {
    unsafe fn swap_remove_erased(&mut self, idx: usize);
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_any(&self) -> &dyn Any;
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

    fn as_any(&self) -> &dyn Any {
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
    pub(crate) data: RwLock<Box<dyn AnyColumn>>,
}

impl ComponentColumn {
    #[doc(hidden)]
    #[allow(private_bounds)]
    pub fn new<T: AnyColumn>(column: T) -> Self {
        Self {
            data: RwLock::new(Box::new(column)),
        }
    }
}

pub struct ComponentColumnRead<'a, T: Component> {
    _gaurd: RwLockReadGuard<'a, Box<dyn AnyColumn>>,
    column: *const Vec<T>,
}

unsafe impl<'a, T: Component> Send for ComponentColumnRead<'a, T> {}
unsafe impl<'a, T: Component> Sync for ComponentColumnRead<'a, T> {}

impl<'a, T: Component> Deref for ComponentColumnRead<'a, T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.column) }
    }
}

pub struct ComponentColumnWrite<'a, T: Component> {
    _gaurd: RwLockWriteGuard<'a, Box<dyn AnyColumn>>,
    column: *mut Vec<T>,
}

unsafe impl<'a, T: Component> Send for ComponentColumnWrite<'a, T> {}
unsafe impl<'a, T: Component> Sync for ComponentColumnWrite<'a, T> {}

impl<'a, T: Component> Deref for ComponentColumnWrite<'a, T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.column) }
    }
}

impl<'a, T: Component> DerefMut for ComponentColumnWrite<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.column) }
    }
}

pub struct Archetype {
    pub(crate) id: ArchetypeId,
    pub(crate) types: IndexSet<TypeId, FxBuildHasher>,
    pub(crate) entities: Vec<Entity>,
    pub(crate) columns: IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
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
            columns,
            type_names,
        }
    }

    pub fn get_column<'a, T: Component>(&self) -> ComponentColumnRead<'a, T> {
        let col = self
            .columns
            .get(&TypeId::of::<T>())
            .unwrap_or_else(|| panic!("Column missing of: {:?}", type_name::<T>()));
        let read_gaurd = col.data.read();
        let vector = read_gaurd
            .as_any()
            .downcast_ref::<Vec<T>>()
            .expect("Type mismatch") as *const Vec<T>;
        let reader = ComponentColumnRead {
            _gaurd: read_gaurd,
            column: vector,
        };
        unsafe { transmute(reader) }
    }

    pub fn get_column_opt<'a, T: Component>(&self) -> Option<ComponentColumnRead<'a, T>> {
        let col = self.columns.get(&TypeId::of::<T>())?;
        let read_gaurd = col.data.read();
        let vector = read_gaurd
            .as_any()
            .downcast_ref::<Vec<T>>()
            .expect("Type mismatch") as *const Vec<T>;
        let opt_read = Some(ComponentColumnRead {
            _gaurd: read_gaurd,
            column: vector,
        });
        unsafe { transmute(opt_read) }
    }

    pub fn get_column_mut<'a, T: Component>(&self) -> ComponentColumnWrite<'a, T> {
        let col = self
            .columns
            .get(&TypeId::of::<T>())
            .unwrap_or_else(|| panic!("Column missing of: {:?}", type_name::<T>()));
        let mut write_gaurd = col.data.write();
        let vector = write_gaurd
            .as_any_mut()
            .downcast_mut::<Vec<T>>()
            .expect("Type mismatch") as *mut Vec<T>;
        let writer = ComponentColumnWrite {
            _gaurd: write_gaurd,
            column: vector,
        };
        unsafe { transmute(writer) }
    }

    pub fn get_column_mut_opt<'a, T: Component>(&self) -> Option<ComponentColumnWrite<'a, T>> {
        let col = self.columns.get(&TypeId::of::<T>())?;
        let mut write_gaurd = col.data.write();
        let vector = write_gaurd
            .as_any_mut()
            .downcast_mut::<Vec<T>>()
            .expect("Type mismatch") as *mut Vec<T>;
        let opt_write = Some(ComponentColumnWrite {
            _gaurd: write_gaurd,
            column: vector,
        });
        unsafe { transmute(opt_write) }
    }

    pub(crate) fn id(&self) -> u32 {
        self.id.0
    }
}

pub(crate) struct ArchetypeManager {
    index: FxHashMap<u64, ArchetypeId>,
    pub(crate) archetypes: Arc<DashMap<ArchetypeId, Archetype, FxBuildHasher>>,
    pub(crate) next_id: u32,
}

impl ArchetypeManager {
    pub(crate) fn new() -> Self {
        Self {
            index: FxHashMap::default(),
            archetypes: Arc::new(DashMap::with_hasher(FxBuildHasher)),
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

    pub(crate) fn get_or_create_from_set<F>(
        &mut self,
        types_set: IndexSet<TypeId, FxBuildHasher>,
        types_names_set: IndexSet<&'static str, FxBuildHasher>,
        setup_columns: F,
    ) -> ArchetypeId
    where
        F: FnOnce(
            &IndexSet<TypeId, FxBuildHasher>,
            &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
        ),
    {
        let order_independent_hash = self.calculate_hash(&types_set);
        if let Some(id) = self.index.get(&order_independent_hash).copied() {
            return id;
        }

        let new_id = ArchetypeId::new(self.next_id);
        self.next_id += 1;
        let mut columns = IndexMap::with_capacity_and_hasher(types_set.len(), FxBuildHasher);
        setup_columns(&types_set, &mut columns);

        let new_arch = Archetype::new(new_id, types_set, columns, types_names_set);

        self.index.insert(order_independent_hash, new_id);
        self.archetypes.insert(new_id, new_arch);
        new_id
    }

    pub(crate) fn get_or_create_from_generic<T: ComponentBundle>(&mut self) -> ArchetypeId {
        let incoming_ids = T::get_type_ids();
        let mut types_set = IndexSet::with_capacity_and_hasher(incoming_ids.len(), FxBuildHasher);
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
            IndexSet::with_capacity_and_hasher(names_ref.len(), FxBuildHasher);
        for &name in names_ref {
            types_names_set.insert(name);
        }

        let new_id = ArchetypeId(self.next_id);
        self.next_id += 1;

        let mut columns = IndexMap::with_hasher(FxBuildHasher);
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

    #[allow(dead_code)]
    pub(crate) fn get<'a>(&'a self, id: ArchetypeId) -> Option<Ref<'a, ArchetypeId, Archetype>> {
        self.archetypes.get(&id)
    }

    pub(crate) fn get_mut<'a>(
        &'a self,
        id: ArchetypeId,
    ) -> Option<RefMut<'a, ArchetypeId, Archetype>> {
        self.archetypes.get_mut(&id)
    }
}
