use fxhash::{FxBuildHasher, FxHashMap};
use indexmap::{IndexMap, IndexSet};
use std::{any::TypeId, sync::atomic::AtomicU32};

#[cfg(feature = "reactivity")]
use crate::reactivity::{REMOVAL_TRACKED_COMPS, TRACKED_COMPONENTS};
use crate::{
    commands::{WorldCommand, bundle::ComponentBundle},
    entity::Entity,
    extensions::{Archetype, ComponentColumn, World},
    registry::{REGISTRY, REGISTRY_HANDLE_COUNT, RegistryData},
    world::archetypes::{AnyColumn, ArchetypeId},
};

pub(crate) struct SpawnCommand<T: ComponentBundle> {
    pub(crate) components: T,
}

impl<T: ComponentBundle> WorldCommand for SpawnCommand<T> {
    fn apply(self, world: &mut World) {
        let arch_id = world.archetypes_manager.get_or_create_from_generic::<T>();
        let next_idx = match world.archetypes_manager.get(arch_id) {
            Some(arch) => arch.entities.len() as u32,
            None => 0,
        };
        let assigned_registry_idx = alloc_registry_cell(arch_id, next_idx, world);
        let arch = world
            .archetypes_manager
            .get_mut(arch_id)
            .expect("Archetype generation failed");

        arch.entities.push(Entity::new(assigned_registry_idx));
        self.components.push_to_archetype(arch);
        #[cfg(feature = "reactivity")]
        unsafe {
            let columns = &mut *arch.columns.get();
            initialize_spawn_markers(columns);
        }
    }
}

pub(crate) struct BatchSpawnCommand<T: ComponentBundle> {
    pub(crate) components_iter: Box<dyn Iterator<Item = T> + Send + 'static>,
}

impl<T: ComponentBundle> WorldCommand for BatchSpawnCommand<T> {
    fn apply(self, world: &mut World) {
        let world_ptr = world as *mut World;
        let arch_id = unsafe {
            (*world_ptr)
                .archetypes_manager
                .get_or_create_from_generic::<T>()
        };

        let arch = unsafe {
            (*world_ptr)
                .archetypes_manager
                .get_mut(arch_id)
                .expect("Archetype generation failed")
        };

        let (lower_bound, _) = self.components_iter.size_hint();
        if lower_bound > 0 {
            arch.entities.reserve(lower_bound);
        }

        #[cfg(feature = "reactivity")]
        let mut batch_size = 0;

        for components in self.components_iter {
            let next_idx = arch.entities.len() as u32;
            let assigned_registry_idx =
                unsafe { alloc_registry_cell(arch_id, next_idx, &mut *world_ptr) };

            arch.entities.push(Entity::new(assigned_registry_idx));
            components.push_to_archetype(arch);

            #[cfg(feature = "reactivity")]
            {
                batch_size += 1;
            }
        }

        #[cfg(feature = "reactivity")]
        if batch_size > 0 {
            unsafe {
                let columns = &mut *arch.columns.get();
                initialize_batch_spawn_markers(columns, batch_size);
            }
        }
    }
}

#[repr(transparent)]
pub struct DespawnCommand {
    pub(crate) entity: Entity,
}

impl DespawnCommand {
    pub(crate) fn apply(&self, world: &mut World) {
        let target_registry_idx = self.entity.registry_index as usize;
        let (arch_id, target_idx) = unsafe {
            let data_ptr = REGISTRY.get_ptr(target_registry_idx);
            let arch_id = (*data_ptr).archetype_id;
            let handle_count = REGISTRY_HANDLE_COUNT.get_count(target_registry_idx);
            if handle_count > 0 {
                let types_names = &&world
                    .archetypes_manager
                    .archetypes
                    .get(&arch_id)
                    .unwrap_unchecked()
                    .type_names;
                panic!(
                    "\n\
                    Avenix Handle Violation: Cloned handles for an entity were not dropped before despawn execution!\n\
                    Entity Archetype Component Names: {:?}\n\
                    Refer to the `DefaultSchedulesPlugin` and `Commands::despawn` documentation to understand how to coordinate handle cleanup.\n\
                    ",
                    types_names
                );
            }

            (arch_id, (*data_ptr).idx)
        };

        let arch = world
            .archetypes_manager
            .get_mut(arch_id)
            .expect("Target archetype missing");
        let last_idx = (arch.entities.len() - 1) as u32;

        if target_idx != last_idx {
            let swapped_entity_registry_idx =
                arch.entities[last_idx as usize].registry_index as usize;

            unsafe {
                let swapped_cell_ptr = REGISTRY.get_mut_ptr(swapped_entity_registry_idx);
                (*swapped_cell_ptr).idx = target_idx;
            }
        }
        unsafe {
            let cols = arch.columns.get();
            for col in (*cols).values_mut() {
                col.data.swap_remove_erased(target_idx as usize);
            }
        }

        arch.entities.swap_remove(target_idx as usize);
        world.free_indices_list.push(target_registry_idx as u32);
    }

    pub fn despawn_target(&self) -> &Entity {
        &self.entity
    }
}

pub(crate) struct AddComponentsCommand<T: ComponentBundle> {
    pub(crate) entity: Entity,
    pub(crate) components: T,
}

impl<T: ComponentBundle> WorldCommand for AddComponentsCommand<T> {
    fn apply(self, world: &mut World) {
        let target_registry_idx = self.entity.registry_index as usize;
        let (old_arch_id, old_idx) = get_registry_location(target_registry_idx);
        let incoming_ids = T::get_type_ids();

        let new_arch_id = if let Some(id) = world
            .archetypes_manager
            .find_target_id_for_addition(old_arch_id, incoming_ids)
        {
            id
        } else {
            create_addition_archetype::<T>(world, old_arch_id, incoming_ids)
        };

        if old_arch_id == new_arch_id {
            return;
        }

        unsafe {
            let (old_arch, new_arch) = get_double_archetypes(world, old_arch_id, new_arch_id);
            {
                let new_cols = &mut *new_arch.columns.get();
                let old_cols = &mut *old_arch.columns.get();
                if new_cols.is_empty() {
                    T::create_empty_columns(new_cols);
                    clone_existing_columns(old_cols, new_cols);
                    #[cfg(feature = "reactivity")]
                    initialize_missing_archetype_markers(&new_arch.types, new_cols);
                }
                move_matching_columns(old_cols, new_cols, old_idx as usize);
            }
            let new_dense_idx = new_arch.entities.len() as u32;
            self.components.push_to_archetype(new_arch);
            #[cfg(feature = "reactivity")]
            {
                let fresh_new_cols = &mut *new_arch.columns.get();
                migrate_addition_markers(&old_arch.types, incoming_ids, fresh_new_cols);
            }
            swap_remove_entity_registry_update(old_arch, old_idx);
            let entity_handle = old_arch.entities.swap_remove(old_idx as usize);
            new_arch.entities.push(entity_handle);
            update_registry_cell(target_registry_idx, new_arch_id, new_dense_idx);
        }
    }
}

pub struct InsertComponentsCommand<T: ComponentBundle> {
    pub entity: Entity,
    pub components: T,
}

impl<T: ComponentBundle> WorldCommand for InsertComponentsCommand<T> {
    fn apply(self, world: &mut World) {
        let target_registry_idx = self.entity.registry_index as usize;
        let (old_arch_id, old_idx) = get_registry_location(target_registry_idx);
        let incoming_ids = T::get_type_ids();

        let new_arch_id = if let Some(id) = world
            .archetypes_manager
            .find_target_id_for_addition(old_arch_id, incoming_ids)
        {
            id
        } else {
            create_addition_archetype::<T>(world, old_arch_id, incoming_ids)
        };

        if old_arch_id == new_arch_id {
            let arch = world.archetypes_manager.get_mut(old_arch_id).expect(
                "Avenix Engine Fatal: Entity registry pointed to an untracked Archetype ID",
            );

            unsafe {
                self.components.insert_to_archetype(arch, old_idx as usize);
            }
            return;
        }

        unsafe {
            let (old_arch, new_arch) = get_double_archetypes(world, old_arch_id, new_arch_id);
            let new_dense_idx = new_arch.entities.len() as u32;

            {
                let new_cols = new_arch.columns.get_mut();
                let old_cols = old_arch.columns.get_mut();

                if new_cols.is_empty() {
                    T::create_empty_columns(new_cols);
                    clone_existing_columns(old_cols, new_cols);
                    #[cfg(feature = "reactivity")]
                    initialize_missing_archetype_markers(&new_arch.types, new_cols);
                }

                move_matching_columns(old_cols, new_cols, old_idx as usize);
            }
            self.components
                .insert_to_archetype(new_arch, new_dense_idx as usize);

            #[cfg(feature = "reactivity")]
            {
                let fresh_new_cols = new_arch.columns.get_mut();
                migrate_addition_markers(&old_arch.types, incoming_ids, fresh_new_cols);
            }

            swap_remove_entity_registry_update(old_arch, old_idx);
            let entity_handle = old_arch.entities.swap_remove(old_idx as usize);
            new_arch.entities.push(entity_handle);
            update_registry_cell(target_registry_idx, new_arch_id, new_dense_idx);
        }
    }
}

pub(crate) struct RemoveComponentsCommand<T: ComponentBundle> {
    pub(crate) entity: Entity,
    pub(crate) _marker: std::marker::PhantomData<T>,
}

impl<T: ComponentBundle> WorldCommand for RemoveComponentsCommand<T> {
    fn apply(self, world: &mut World) {
        let target_registry_idx = self.entity.registry_index as usize;
        let (old_arch_id, old_idx) = get_registry_location(target_registry_idx);
        let removed_ids = T::get_type_ids();

        let new_arch_id = if let Some(id) = world
            .archetypes_manager
            .find_target_id_for_subtraction(old_arch_id, removed_ids)
        {
            id
        } else {
            create_subtraction_archetype::<T>(world, old_arch_id, removed_ids)
        };

        if old_arch_id == new_arch_id {
            return;
        }

        unsafe {
            #[cfg(feature = "reactivity")]
            let world_ptr = world as *mut World;
            let (old_arch, new_arch) = get_double_archetypes(world, old_arch_id, new_arch_id);
            let old_cols = &mut *old_arch.columns.get();
            let new_cols = &mut *new_arch.columns.get();
            if new_cols.is_empty() {
                populate_subtracted_columns(&new_arch.types, old_cols, new_cols);
            }
            let new_dense_idx = new_arch.entities.len() as u32;
            move_matching_columns(old_cols, new_cols, old_idx as usize);
            erase_subtracted_columns(removed_ids, old_cols, old_idx as usize);

            #[cfg(feature = "reactivity")]
            {
                erase_subtracted_markers(
                    &old_arch.types,
                    &new_arch.types,
                    old_cols,
                    old_idx as usize,
                );
                let entity_handle = &old_arch.entities[old_idx as usize];
                push_removed_components_reactivity(world_ptr, removed_ids, entity_handle);
            }

            swap_remove_entity_registry_update(old_arch, old_idx);

            let entity_handle = old_arch.entities.swap_remove(old_idx as usize);
            new_arch.entities.push(entity_handle);

            update_registry_cell(target_registry_idx, new_arch_id, new_dense_idx);
        }
    }
}

#[inline(always)]
fn get_registry_location(registry_idx: usize) -> (ArchetypeId, u32) {
    unsafe {
        let cell_ptr = REGISTRY.get_ptr(registry_idx);
        let arch_id = (*cell_ptr).archetype_id;
        (arch_id, (*cell_ptr).idx)
    }
}

#[inline(always)]
fn update_registry_cell(registry_idx: usize, archetype_id: ArchetypeId, dense_idx: u32) {
    unsafe {
        let cell_ptr = REGISTRY.get_mut_ptr(registry_idx);
        (*cell_ptr).archetype_id = archetype_id;
        (*cell_ptr).idx = dense_idx;
    }
}

#[inline(always)]
fn alloc_registry_cell(archetype_id: ArchetypeId, dense_idx: u32, world: &mut World) -> u32 {
    if let Some(recycled_idx) = world.free_indices_list.pop() {
        unsafe {
            let data_ptr = REGISTRY.get_mut_ptr(recycled_idx as usize);
            (*data_ptr) = RegistryData {
                archetype_id,
                idx: dense_idx,
            };
            let count_ptr = REGISTRY_HANDLE_COUNT.get_mut_ptr(recycled_idx as usize);
            (*count_ptr) = AtomicU32::new(0);
        }
        recycled_idx
    } else {
        let len = REGISTRY.len() as u32;
        REGISTRY.push(RegistryData {
            archetype_id,
            idx: dense_idx,
        });
        REGISTRY_HANDLE_COUNT.push(AtomicU32::new(0));
        len
    }
}

#[inline(always)]
unsafe fn get_double_archetypes(
    world: &mut World,
    old_id: ArchetypeId,
    new_id: ArchetypeId,
) -> (&mut Archetype, &mut Archetype) {
    let map_ptr =
        &mut world.archetypes_manager.archetypes as *mut FxHashMap<ArchetypeId, Archetype>;
    let old_arch = unsafe { (*map_ptr).get_mut(&old_id).expect("Old archetype missing") };
    let new_arch = unsafe { (*map_ptr).get_mut(&new_id).expect("New archetype missing") };
    (old_arch, new_arch)
}

fn create_addition_archetype<T: ComponentBundle>(
    world: &mut World,
    old_arch_id: ArchetypeId,
    incoming_ids: &[TypeId],
) -> ArchetypeId {
    let old_arch = world
        .archetypes_manager
        .archetypes
        .get(&old_arch_id)
        .unwrap();
    let mut new_types = old_arch.types.clone();
    for id in incoming_ids {
        new_types.insert(*id);
    }

    #[cfg(feature = "reactivity")]
    world
        .archetypes_manager
        .sync_tracking_markers(&mut new_types);

    let mut new_types_names = old_arch.type_names.clone();
    for id in T::get_type_names().as_ref() {
        new_types_names.insert(id);
    }
    world
        .archetypes_manager
        .get_or_create_from_set(new_types, new_types_names)
}

fn create_subtraction_archetype<T: ComponentBundle>(
    world: &mut World,
    old_arch_id: ArchetypeId,
    removed_ids: &[TypeId],
) -> ArchetypeId {
    let old_arch = world
        .archetypes_manager
        .archetypes
        .get(&old_arch_id)
        .unwrap();
    let mut new_types = old_arch.types.clone();
    for id in removed_ids {
        new_types.swap_remove(id);
    }

    #[cfg(feature = "reactivity")]
    world
        .archetypes_manager
        .sync_tracking_markers(&mut new_types);

    let mut new_types_names = old_arch.type_names.clone();
    for id_name in T::get_type_names().as_ref() {
        new_types_names.shift_remove(id_name);
    }
    world
        .archetypes_manager
        .get_or_create_from_set(new_types, new_types_names)
}

#[inline(always)]
fn clone_existing_columns(
    src: &IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
    dst: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
) {
    for (type_id, old_col) in src.iter() {
        if !dst.contains_key(type_id) {
            dst.insert(
                *type_id,
                ComponentColumn {
                    data: old_col.data.clone_empty(),
                },
            );
        }
    }
}

#[inline(always)]
fn populate_subtracted_columns(
    allowed_types: &IndexSet<TypeId, FxBuildHasher>,
    src: &IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
    dst: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
) {
    for (type_id, old_col) in src.iter() {
        if allowed_types.contains(type_id) {
            dst.insert(
                *type_id,
                ComponentColumn {
                    data: old_col.data.clone_empty(),
                },
            );
        }
    }
}

#[inline(always)]
unsafe fn move_matching_columns(
    src: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
    dst: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
    row_idx: usize,
) {
    for (type_id, old_col) in src.iter_mut() {
        if let Some(new_col) = dst.get_mut(type_id) {
            let dst_ptr = &mut *new_col.data as *mut dyn AnyColumn;
            unsafe { old_col.data.move_row_erased(row_idx, dst_ptr) };
        }
    }
}

#[inline(always)]
unsafe fn erase_subtracted_columns(
    ids: &[TypeId],
    columns: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
    row_idx: usize,
) {
    for id in ids {
        if let Some(col) = columns.get_mut(id) {
            unsafe { col.data.swap_remove_erased(row_idx) };
        }
    }
}

#[inline(always)]
unsafe fn swap_remove_entity_registry_update(arch: &mut Archetype, removed_row_idx: u32) {
    let last_idx = (arch.entities.len() - 1) as u32;
    if removed_row_idx != last_idx {
        let swapped_entity_registry_idx = arch.entities[last_idx as usize].registry_index as usize;
        let swapped_cell = unsafe { REGISTRY.get_mut_ptr(swapped_entity_registry_idx) };
        unsafe {
            (*swapped_cell).idx = removed_row_idx;
        }
    }
}

#[cfg(feature = "reactivity")]
#[inline(always)]
fn initialize_spawn_markers(columns: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>) {
    let tracked = TRACKED_COMPONENTS.read();
    for meta in tracked.values() {
        if let Some(marker_column) = columns.get_mut(&meta.marker_id) {
            unsafe { (meta.push_default_marker)(marker_column) };
        }
    }
}

#[cfg(feature = "reactivity")]
#[inline]
fn initialize_batch_spawn_markers(
    columns: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
    batch_size: usize,
) {
    let tracked = TRACKED_COMPONENTS.read();
    for meta in tracked.values() {
        if let Some(marker_column) = columns.get_mut(&meta.marker_id) {
            for _ in 0..batch_size {
                unsafe { (meta.push_default_marker)(marker_column) };
            }
        }
    }
}

#[cfg(feature = "reactivity")]
#[inline(always)]
fn initialize_missing_archetype_markers(
    types: &IndexSet<TypeId, FxBuildHasher>,
    columns: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
) {
    let tracked = TRACKED_COMPONENTS.read();
    for meta in tracked.values() {
        if types.contains(&meta.marker_id) && !columns.contains_key(&meta.marker_id) {
            columns.insert(meta.marker_id, (meta.create_marker_column)());
        }
    }
}

#[cfg(feature = "reactivity")]
#[inline(always)]
unsafe fn migrate_addition_markers(
    old_types: &IndexSet<TypeId, FxBuildHasher>,
    incoming_ids: &[TypeId],
    new_cols: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
) {
    let tracked = TRACKED_COMPONENTS.read();
    for meta in tracked.values() {
        if new_cols.contains_key(&meta.marker_id) {
            if old_types.contains(&meta.marker_id) {
                continue;
            } else if incoming_ids.contains(&meta.component_id)
                && let Some(marker_column) = new_cols.get_mut(&meta.marker_id)
            {
                unsafe { (meta.push_default_marker)(marker_column) };
            }
        }
    }
}

#[cfg(feature = "reactivity")]
#[inline(always)]
unsafe fn erase_subtracted_markers(
    old_types: &IndexSet<TypeId, FxBuildHasher>,
    new_types: &IndexSet<TypeId, FxBuildHasher>,
    old_cols: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
    row_idx: usize,
) {
    let tracked = TRACKED_COMPONENTS.read();
    for meta in tracked.values() {
        if old_types.contains(&meta.marker_id)
            && !new_types.contains(&meta.marker_id)
            && let Some(marker_col) = old_cols.get_mut(&meta.marker_id)
        {
            unsafe { marker_col.data.swap_remove_erased(row_idx) };
        }
    }
}

#[cfg(feature = "reactivity")]
pub(crate) unsafe fn push_removed_components_reactivity(
    world_ptr: *mut World,
    removed_ids: &[std::any::TypeId],
    entity: &Entity,
) {
    if removed_ids.is_empty() {
        return;
    }
    let tracked = REMOVAL_TRACKED_COMPS.read();
    if tracked.is_empty() {
        return;
    }
    let world = unsafe { &mut *world_ptr };

    for type_id in removed_ids {
        if let Some(meta) = tracked.get(type_id) {
            (meta.push_to_write_queue)(world, entity.clone());
        }
    }
}
