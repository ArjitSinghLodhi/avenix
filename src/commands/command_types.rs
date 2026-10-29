use dashmap::mapref::one::Ref;
use indexmap::IndexMap;
use indexmap::IndexSet;
use parking_lot::MutexGuard;
use parking_lot::{Mutex, RwLock, RwLockWriteGuard};
use rustc_hash::FxBuildHasher;
use std::any::TypeId;
use std::marker::PhantomData;

use crate::entity::ENTITY_HANDLE_COUNT_REGISTRY;
use crate::entity::ENTITY_LOCATION_REGISTRY;
use crate::entity::HandleCount;
use crate::entity::RegistryData;
use crate::reactivity::TrackedComponentMeta;
use crate::reactivity::{TRACKED_COMPONENTS, removed::REMOVAL_TRACKED_COMPS};
use crate::{
    commands::{Command, bundle::ComponentBundle},
    component::hooks::{ExclusiveWorld, GLOBAL_HOOK_REGISTRY},
    entity::Entity,
    world::{
        World,
        archetypes::{AnyColumn, Archetype, ArchetypeId, ComponentColumn},
    },
};

pub(crate) struct SpawnCommand<T: ComponentBundle> {
    pub(crate) components: T,
}

impl<T: ComponentBundle> Command for SpawnCommand<T> {
    fn apply(self, world: &mut World) {
        let arch_id = world.archetypes_manager.get_or_create_from_generic::<T>();
        let next_idx = match world.archetypes_manager.get_mut(arch_id) {
            Some(arch) => arch.entities.read().len() as u32,
            None => 0,
        };
        let assigned_registry_idx = alloc_registry_cell(arch_id, next_idx, world);
        let mut arch = world
            .archetypes_manager
            .get_mut(arch_id)
            .expect("Archetype generation failed");
        let entity = Entity::new(assigned_registry_idx);
        let row_idx = {
            let mut entities_lock = arch.entities.write();
            let row_idx = entities_lock.len();
            entities_lock.push(entity.clone());
            row_idx
        };
        {
            let columns = &mut arch.columns;
            initialize_spawn_markers(&TRACKED_COMPONENTS.lock(), columns);
        }
        unsafe { self.components.insert_to_archetype(&mut arch, row_idx) };

        drop(arch);

        let type_ids = T::get_type_ids();
        let mut deferred_world = ExclusiveWorld { world };

        let mut must_register = false;
        {
            let read_guard = GLOBAL_HOOK_REGISTRY.read();
            for &id in type_ids {
                if read_guard.get_hooks(id).is_none() {
                    must_register = true;
                    break;
                }
            }
        }

        if must_register {
            T::register_bundle_hooks();
        }

        let mut active_hooks = Vec::with_capacity(type_ids.len());
        {
            let read_guard = GLOBAL_HOOK_REGISTRY.read();
            for &id in type_ids {
                active_hooks.push(read_guard.get_hooks(id));
            }
        }

        for hooks in active_hooks.into_iter().flatten() {
            if let Some(on_add_hook) = hooks.on_add {
                on_add_hook(&mut deferred_world, &entity);
            }
            if let Some(on_insert_hook) = hooks.on_insert {
                on_insert_hook(&mut deferred_world, &entity);
            }
        }
    }
}

pub(crate) struct BatchSpawnCommand<T, I>
where
    T: ComponentBundle,
    I: Iterator<Item = T> + Send + 'static,
{
    pub(crate) components_iter: I,
}

impl<T: ComponentBundle, I: Iterator<Item = T> + Send + 'static> Command
    for BatchSpawnCommand<T, I>
{
    fn apply(self, world: &mut World) {
        let world_ptr = world as *mut World;
        let arch_id = unsafe {
            (*world_ptr)
                .archetypes_manager
                .get_or_create_from_generic::<T>()
        };

        let mut arch = unsafe {
            (*world_ptr)
                .archetypes_manager
                .get_mut(arch_id)
                .expect("Archetype generation failed")
        };

        let (lower_bound, _) = self.components_iter.size_hint();
        if lower_bound > 0 {
            arch.entities.write().reserve(lower_bound);
        }

        let mut spawned_entities = Vec::new();
        if lower_bound > 0 {
            spawned_entities.reserve(lower_bound);
        }
        let tracked_components_guard = TRACKED_COMPONENTS.lock();
        for components in self.components_iter {
            let entity = {
                let mut arch_entities = arch.entities.write();
                let next_idx = arch_entities.len() as u32;
                let assigned_registry_idx =
                    unsafe { alloc_registry_cell(arch_id, next_idx, &mut *world_ptr) };
                let entity = Entity::new(assigned_registry_idx);
                arch_entities.push(entity.clone());
                entity
            };

            let row_idx = arch.entities.read().len() - 1;
            initialize_spawn_markers(&tracked_components_guard, &mut arch.columns);
            unsafe { components.insert_to_archetype(&mut arch, row_idx) };
            spawned_entities.push(entity);
        }

        drop(arch);

        let type_ids = T::get_type_ids();
        let mut exclusive_world = ExclusiveWorld { world };

        let mut must_register = false;
        {
            let read_guard = GLOBAL_HOOK_REGISTRY.read();
            for &id in type_ids {
                if read_guard.get_hooks(id).is_none() {
                    must_register = true;
                    break;
                }
            }
        }

        if must_register {
            T::register_bundle_hooks();
        }

        let mut active_hooks = Vec::with_capacity(type_ids.len());
        {
            let read_guard = GLOBAL_HOOK_REGISTRY.read();
            for &id in type_ids {
                active_hooks.push(read_guard.get_hooks(id));
            }
        }

        for entity in &spawned_entities {
            for hooks in active_hooks.iter().flatten() {
                if let Some(on_add_hook) = hooks.on_add {
                    on_add_hook(&mut exclusive_world, entity);
                }
                if let Some(on_insert_hook) = hooks.on_insert {
                    on_insert_hook(&mut exclusive_world, entity);
                }
            }
        }
    }
}

const ENTITY_HANDLE_VIOLATION_ERR: &str = "AvenixEntityHandleViolation";

pub(crate) struct DespawnCommand {
    pub(crate) entity: Entity,
}

impl Command for DespawnCommand {
    fn apply(self, world: &mut World) {
        let target_registry_idx = self.entity.registry_index() as usize;
        unsafe {
            (*ENTITY_HANDLE_COUNT_REGISTRY.get_ptr(target_registry_idx)).decrement();
        }

        let initial_arch_id = unsafe {
            let data_ptr = ENTITY_LOCATION_REGISTRY
                .read()
                .as_ptr()
                .add(target_registry_idx);
            (*data_ptr).archetype_id()
        };

        let arch = world
            .archetypes_manager
            .get_mut(initial_arch_id)
            .expect("Target archetype missing");
        let mut active_hooks = Vec::with_capacity(arch.types.len());
        {
            let read_guard = GLOBAL_HOOK_REGISTRY.read();
            for &id in arch.types.iter() {
                active_hooks.push(read_guard.get_hooks(id));
            }
        }
        drop(arch);

        let mut exclusive_world = ExclusiveWorld { world };

        for hooks_opt in active_hooks {
            if let Some(hooks) = hooks_opt
                && let Some(on_remove_hook) = hooks.on_remove
            {
                on_remove_hook(&mut exclusive_world, &self.entity);
            }
        }

        let handle_count =
            (unsafe { &*ENTITY_HANDLE_COUNT_REGISTRY.get_ptr(target_registry_idx) }).get_count();
        if handle_count > 0 {
            let arch_mutex = &world
                .archetypes_manager
                .archetypes
                .get(&initial_arch_id)
                .unwrap();
            let lock = arch_mutex.lock();
            let types_names = &lock.type_names;
            panic!(
                "{}\n\
                Cloned handles for an entity were not dropped in on_remove hook execution and before handle count check!\n\
                Entity Archetype Component Names: \n{:#?}\n\
                Refer to the `on_remove` hook inside the Component trait documentation to understand how to coordinate handle cleanup.\n\
                ",
                ENTITY_HANDLE_VIOLATION_ERR, types_names
            );
        }

        let mut entity_location_guard = ENTITY_LOCATION_REGISTRY.write();
        let (current_arch_id, current_target_idx) = unsafe {
            let data_ptr = entity_location_guard.as_ptr().add(target_registry_idx);
            ((*data_ptr).archetype_id(), (*data_ptr).index())
        };

        let mut arch = world
            .archetypes_manager
            .get_mut(current_arch_id)
            .expect("Target archetype missing");

        let last_idx = (arch.entities.read().len() - 1) as u32;

        if current_target_idx != last_idx {
            let swapped_entity_registry_idx =
                arch.entities.read()[last_idx as usize].registry_index() as usize;

            unsafe {
                let swapped_cell_ptr = entity_location_guard
                    .as_mut_ptr()
                    .add(swapped_entity_registry_idx);
                (*swapped_cell_ptr).set_index(current_target_idx);
            }
        }
        unsafe {
            let cols = &mut arch.columns;
            for col in cols.values_mut() {
                col.data
                    .write()
                    .swap_remove_erased(current_target_idx as usize);
            }
        }

        arch.entities
            .write()
            .swap_remove(current_target_idx as usize);
        world.free_indices_list.push(target_registry_idx as u32);
    }
}

pub(crate) struct InsertComponentsCommand<T: ComponentBundle> {
    pub(crate) entity: Entity,
    pub(crate) components: T,
}

impl<T: ComponentBundle> Command for InsertComponentsCommand<T> {
    fn apply(self, world: &mut World) {
        let incoming_ids = T::get_type_ids();
        let target_registry_idx = self.entity.registry_index() as usize;

        let mut exclusive_world = ExclusiveWorld { world };

        let mut must_register = false;
        {
            let read_guard = GLOBAL_HOOK_REGISTRY.read();
            for &id in incoming_ids {
                if read_guard.get_hooks(id).is_none() {
                    must_register = true;
                    break;
                }
            }
        }

        if must_register {
            T::register_bundle_hooks();
        }

        let (mut old_arch_id, mut old_idx) = get_registry_location(target_registry_idx);

        let mut new_arch_id = if let Some(id) = exclusive_world
            .world
            .archetypes_manager
            .find_target_id_for_addition(old_arch_id, incoming_ids)
        {
            id
        } else {
            create_addition_archetype::<T>(exclusive_world.world, old_arch_id, incoming_ids)
        };

        if old_arch_id == new_arch_id {
            let mut active_hooks = Vec::with_capacity(incoming_ids.len());
            {
                let read_guard = GLOBAL_HOOK_REGISTRY.read();
                for &id in incoming_ids {
                    active_hooks.push(read_guard.get_hooks(id));
                }
            }

            for hooks_opt in &active_hooks {
                if let Some(hooks) = hooks_opt
                    && let Some(on_discard_hook) = hooks.on_discard
                {
                    on_discard_hook(&mut exclusive_world, &self.entity);
                }
            }

            let (fresh_arch_id, fresh_idx) = get_registry_location(target_registry_idx);

            let mut arch = exclusive_world
                .world
                .archetypes_manager
                .get_mut(fresh_arch_id)
                .expect(
                    "Avenix Engine Fatal: Entity registry pointed to an untracked Archetype ID",
                );

            unsafe {
                self.components
                    .insert_to_archetype(&mut arch, fresh_idx as usize);
            }

            drop(arch);

            for hooks_opt in active_hooks {
                if let Some(hooks) = hooks_opt
                    && let Some(on_insert_hook) = hooks.on_insert
                {
                    on_insert_hook(&mut exclusive_world, &self.entity);
                }
            }
            return;
        }

        let mut old_has_component = Vec::with_capacity(incoming_ids.len());
        {
            let map = &exclusive_world.world.archetypes_manager.archetypes;
            let old_arch_mutex = map.get(&old_arch_id).expect("Old archetype missing");
            let old_arch = old_arch_mutex.lock();
            for &id in incoming_ids {
                old_has_component.push(old_arch.types.contains(&id));
            }
        }

        let mut active_hooks = Vec::with_capacity(incoming_ids.len());
        {
            let read_guard = GLOBAL_HOOK_REGISTRY.read();
            for &id in incoming_ids {
                active_hooks.push(read_guard.get_hooks(id));
            }
        }

        for (i, hooks_opt) in active_hooks.iter().enumerate() {
            if old_has_component[i]
                && let Some(hooks) = hooks_opt
                && let Some(on_discard_hook) = hooks.on_discard
            {
                on_discard_hook(&mut exclusive_world, &self.entity);
            }
        }

        let (fresh_arch_id, fresh_idx) = get_registry_location(target_registry_idx);

        if fresh_arch_id != old_arch_id {
            old_arch_id = fresh_arch_id;
            old_idx = fresh_idx;
            new_arch_id = if let Some(id) = exclusive_world
                .world
                .archetypes_manager
                .find_target_id_for_addition(old_arch_id, incoming_ids)
            {
                id
            } else {
                create_addition_archetype::<T>(exclusive_world.world, old_arch_id, incoming_ids)
            };
        }

        unsafe {
            let (old_arch_mutex, new_arch_mutex) =
                get_double_archetypes(exclusive_world.world, old_arch_id, new_arch_id);
            let mut old_arch = old_arch_mutex.lock();
            let mut new_arch = new_arch_mutex.lock();
            let new_dense_idx = {
                let Archetype {
                    columns: old_cols, ..
                } = &mut *old_arch;
                let Archetype {
                    entities: new_arch_entities,
                    columns: new_cols,
                    ..
                } = &mut *new_arch;
                let new_arch_entities = new_arch_entities.read();
                move_matching_columns(old_cols, new_cols, old_idx as usize);
                new_arch_entities.len() as u32
            };

            let fresh_new_cols = &mut new_arch.columns;
            migrate_addition_markers(&old_arch.types, incoming_ids, fresh_new_cols);

            self.components
                .insert_to_archetype(&mut new_arch, new_dense_idx as usize);

            let mut old_arch_entities = old_arch.entities.write();
            swap_remove_entity_registry_update(&mut old_arch_entities, old_idx);
            let entity_handle = old_arch_entities.swap_remove(old_idx as usize);
            new_arch.entities.write().push(entity_handle);
            update_registry_cell(target_registry_idx, new_arch_id, new_dense_idx);
        }

        let mut fresh_active_hooks = Vec::with_capacity(incoming_ids.len());
        {
            let read_guard = GLOBAL_HOOK_REGISTRY.read();
            for &id in incoming_ids {
                fresh_active_hooks.push(read_guard.get_hooks(id));
            }
        }

        for (i, hooks_opt) in fresh_active_hooks.into_iter().enumerate() {
            if let Some(hooks) = hooks_opt {
                if !old_has_component[i]
                    && let Some(on_add_hook) = hooks.on_add
                {
                    on_add_hook(&mut exclusive_world, &self.entity);
                }
                if let Some(on_insert_hook) = hooks.on_insert {
                    on_insert_hook(&mut exclusive_world, &self.entity);
                }
            }
        }
    }
}

pub(crate) struct RemoveComponentsCommand<T: ComponentBundle> {
    pub(crate) entity: Entity,
    pub(crate) _marker: PhantomData<T>,
}

impl<T: ComponentBundle> Command for RemoveComponentsCommand<T> {
    fn apply(self, world: &mut World) {
        let target_registry_idx = self.entity.registry_index() as usize;
        let removed_ids = T::get_type_ids();

        let mut exclusive_world = ExclusiveWorld { world };

        let mut must_register = false;
        {
            let read_guard = GLOBAL_HOOK_REGISTRY.read();
            for &id in removed_ids {
                if read_guard.get_hooks(id).is_none() {
                    must_register = true;
                    break;
                }
            }
        }

        if must_register {
            T::register_bundle_hooks();
        }

        let mut active_hooks = Vec::with_capacity(removed_ids.len());
        {
            let read_guard = GLOBAL_HOOK_REGISTRY.read();
            for &id in removed_ids {
                active_hooks.push(read_guard.get_hooks(id));
            }
        }

        for hooks_opt in active_hooks {
            if let Some(hooks) = hooks_opt
                && let Some(on_remove_hook) = hooks.on_remove
            {
                on_remove_hook(&mut exclusive_world, &self.entity);
            }
        }

        let (old_arch_id, old_idx) = get_registry_location(target_registry_idx);

        let new_arch_id = if let Some(id) = exclusive_world
            .world
            .archetypes_manager
            .find_target_id_for_subtraction(old_arch_id, removed_ids)
        {
            id
        } else {
            create_subtraction_archetype::<T>(exclusive_world.world, old_arch_id, removed_ids)
        };

        if old_arch_id == new_arch_id {
            return;
        }

        unsafe {
            let world_ptr = exclusive_world.world as *mut World;
            let (old_arch_mutex, new_arch_mutex) =
                get_double_archetypes(exclusive_world.world, old_arch_id, new_arch_id);
            let mut old_arch = old_arch_mutex.lock();
            let mut new_arch = new_arch_mutex.lock();
            let Archetype {
                entities: new_arch_entities,
                columns: new_cols,
                types: new_types,
                ..
            } = &mut *new_arch;
            let mut new_arch_entities = new_arch_entities.write();
            let new_dense_idx = new_arch_entities.len() as u32;
            let Archetype {
                entities: old_arch_entities,
                columns: old_cols,
                types: old_types,
                ..
            } = &mut *old_arch;
            move_matching_columns(old_cols, new_cols, old_idx as usize);
            erase_subtracted_columns(removed_ids, old_cols, old_idx as usize);

            let mut old_arch_entities = old_arch_entities.write();

            erase_subtracted_markers(old_types, new_types, old_cols, old_idx as usize);
            let entity_handle = &old_arch_entities[old_idx as usize];
            push_removed_components_reactivity(world_ptr, removed_ids, entity_handle);

            swap_remove_entity_registry_update(&mut old_arch_entities, old_idx);

            let entity_handle = old_arch_entities.swap_remove(old_idx as usize);
            new_arch_entities.push(entity_handle);

            update_registry_cell(target_registry_idx, new_arch_id, new_dense_idx);
        }
    }
}

#[inline(always)]
fn get_registry_location(registry_idx: usize) -> (ArchetypeId, u32) {
    unsafe {
        let cell_ptr = ENTITY_LOCATION_REGISTRY.read().as_ptr().add(registry_idx);
        let arch_id = (*cell_ptr).archetype_id();
        (arch_id, (*cell_ptr).index())
    }
}

#[inline(always)]
fn update_registry_cell(registry_idx: usize, archetype_id: ArchetypeId, dense_idx: u32) {
    unsafe {
        let mut lock = ENTITY_LOCATION_REGISTRY.write();
        let cell_ptr = lock.as_mut_ptr().add(registry_idx);
        (*cell_ptr).set_archetype_id(archetype_id);
        (*cell_ptr).set_index(dense_idx);
    }
}

#[inline(always)]
fn alloc_registry_cell(archetype_id: ArchetypeId, dense_idx: u32, world: &mut World) -> u32 {
    let mut lock = ENTITY_LOCATION_REGISTRY.write();
    if let Some(recycled_idx) = world.free_indices_list.pop() {
        unsafe {
            let data_ptr = lock.as_mut_ptr().add(recycled_idx as usize);
            (*data_ptr).set_archetype_id(archetype_id);
            (*data_ptr).set_index(dense_idx);
            let count_ptr = ENTITY_HANDLE_COUNT_REGISTRY.get_ptr(recycled_idx as usize);
            (*count_ptr).set_count(0);
        }
        recycled_idx
    } else {
        let len = lock.len() as u32;
        lock.push(RegistryData::new(archetype_id, dense_idx));
        ENTITY_HANDLE_COUNT_REGISTRY.push(HandleCount::new(0));
        len
    }
}

#[inline(always)]
unsafe fn get_double_archetypes(
    world: &mut World,
    old_id: ArchetypeId,
    new_id: ArchetypeId,
) -> (
    Ref<'_, ArchetypeId, Mutex<Archetype>>,
    Ref<'_, ArchetypeId, Mutex<Archetype>>,
) {
    let map = &mut world.archetypes_manager.archetypes;
    let old_arch = map.get(&old_id).expect("Old archetype missing");
    let new_arch = map.get(&new_id).expect("New archetype missing");
    (old_arch, new_arch)
}

fn create_addition_archetype<T: ComponentBundle>(
    world: &mut World,
    old_arch_id: ArchetypeId,
    incoming_ids: &[TypeId],
) -> ArchetypeId {
    let mut new_types_names;
    let mut new_types;

    let mut cloned_base_cols = IndexMap::with_hasher(FxBuildHasher);
    {
        let old_arch_mutex = world
            .archetypes_manager
            .archetypes
            .get(&old_arch_id)
            .expect("Avenix Engine Fatal: Old Archetype ID not found");
        let old_arch = old_arch_mutex.lock();
        new_types = old_arch.types.clone();
        for id in incoming_ids {
            new_types.insert(*id);
        }

        world
            .archetypes_manager
            .sync_tracking_markers(&mut new_types);

        new_types_names = old_arch.type_names.clone();
        for id in T::get_type_names().as_ref() {
            new_types_names.insert(id);
        }

        for (type_id, old_col) in old_arch.columns.iter() {
            cloned_base_cols.insert(
                *type_id,
                ComponentColumn {
                    data: RwLock::new(old_col.data.write().clone_empty()),
                },
            );
        }
    }

    world.archetypes_manager.get_or_create_from_set(
        new_types,
        new_types_names,
        |_types_set, cols| {
            T::create_empty_columns(cols);

            for (type_id, col) in cloned_base_cols {
                cols.entry(type_id).or_insert(col);
            }

            initialize_missing_archetype_markers(_types_set, cols);
        },
    )
}

fn create_subtraction_archetype<T: ComponentBundle>(
    world: &mut World,
    old_arch_id: ArchetypeId,
    removed_ids: &[TypeId],
) -> ArchetypeId {
    let mut new_types;
    let mut new_types_names;
    let mut cloned_base_cols = IndexMap::with_hasher(FxBuildHasher);
    {
        let old_arch_mutex = world
            .archetypes_manager
            .archetypes
            .get(&old_arch_id)
            .expect("Avenix Engine Fatal: Old Archetype ID not found");
        let old_arch = old_arch_mutex.lock();
        new_types = old_arch.types.clone();
        for id in removed_ids {
            new_types.swap_remove(id);
        }

        world
            .archetypes_manager
            .sync_tracking_markers(&mut new_types);

        new_types_names = old_arch.type_names.clone();
        for id_name in T::get_type_names().as_ref() {
            new_types_names.shift_remove(id_name);
        }

        for (type_id, old_col) in old_arch.columns.iter() {
            if new_types.contains(type_id) {
                cloned_base_cols.insert(
                    *type_id,
                    ComponentColumn {
                        data: RwLock::new(old_col.data.write().clone_empty()),
                    },
                );
            }
        }
    }

    world.archetypes_manager.get_or_create_from_set(
        new_types,
        new_types_names,
        |_types_set, cols| {
            for (type_id, col) in cloned_base_cols {
                cols.insert(type_id, col);
            }

            initialize_missing_archetype_markers(_types_set, cols);
        },
    )
}

#[inline(always)]
unsafe fn move_matching_columns(
    src: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
    dst: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
    row_idx: usize,
) {
    for (type_id, old_col) in src.iter_mut() {
        if let Some(new_col) = dst.get_mut(type_id) {
            let mut data_guard = new_col.data.write();
            let dst_ptr = &mut **data_guard as *mut dyn AnyColumn;
            unsafe { old_col.data.write().move_row_erased(row_idx, dst_ptr) };
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
            unsafe { col.data.write().swap_remove_erased(row_idx) };
        }
    }
}

#[inline(always)]
unsafe fn swap_remove_entity_registry_update(
    arch_entities: &mut RwLockWriteGuard<Vec<Entity>>,
    removed_row_idx: u32,
) {
    let mut lock = ENTITY_LOCATION_REGISTRY.write();
    let last_idx = (arch_entities.len() - 1) as u32;
    if removed_row_idx != last_idx {
        let swapped_entity_registry_idx =
            arch_entities[last_idx as usize].registry_index() as usize;
        let swapped_cell = unsafe { lock.as_mut_ptr().add(swapped_entity_registry_idx) };
        unsafe {
            (*swapped_cell).set_index(removed_row_idx);
        }
    }
}

#[inline(always)]
fn initialize_spawn_markers(
    tracked_components_guard: &MutexGuard<
        '_,
        IndexMap<TypeId, Vec<TrackedComponentMeta>, FxBuildHasher>,
    >,
    columns: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
) {
    for meta_list in tracked_components_guard.values() {
        for meta in meta_list {
            if let Some(marker_column) = columns.get_mut(&meta.marker_id) {
                unsafe { (meta.push_default_marker)(marker_column) };
            }
        }
    }
}

#[inline(always)]
fn initialize_missing_archetype_markers(
    types: &IndexSet<TypeId, FxBuildHasher>,
    columns: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
) {
    let tracked = TRACKED_COMPONENTS.lock();
    for meta_list in tracked.values() {
        for meta in meta_list {
            if types.contains(&meta.marker_id) && !columns.contains_key(&meta.marker_id) {
                columns.insert(meta.marker_id, (meta.create_marker_column)());
            }
        }
    }
}

#[inline(always)]
unsafe fn migrate_addition_markers(
    old_types: &IndexSet<TypeId, FxBuildHasher>,
    incoming_ids: &[TypeId],
    new_cols: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
) {
    let tracked = TRACKED_COMPONENTS.lock();

    for &comp_id in incoming_ids {
        if let Some(meta_list) = tracked.get(&comp_id) {
            for meta in meta_list {
                if new_cols.contains_key(&meta.marker_id) {
                    if old_types.contains(&meta.marker_id) {
                        continue;
                    } else if let Some(marker_column) = new_cols.get_mut(&meta.marker_id) {
                        unsafe { (meta.push_default_marker)(marker_column) };
                    }
                }
            }
        }
    }
}

#[inline(always)]
unsafe fn erase_subtracted_markers(
    old_types: &IndexSet<TypeId, FxBuildHasher>,
    new_types: &IndexSet<TypeId, FxBuildHasher>,
    old_cols: &mut IndexMap<TypeId, ComponentColumn, FxBuildHasher>,
    row_idx: usize,
) {
    let tracked = TRACKED_COMPONENTS.lock();
    for meta_list in tracked.values() {
        for meta in meta_list {
            if old_types.contains(&meta.marker_id)
                && !new_types.contains(&meta.marker_id)
                && let Some(marker_col) = old_cols.get_mut(&meta.marker_id)
            {
                unsafe { marker_col.data.write().swap_remove_erased(row_idx) };
            }
        }
    }
}

pub(crate) unsafe fn push_removed_components_reactivity(
    world_ptr: *mut World,
    removed_ids: &[std::any::TypeId],
    entity: &Entity,
) {
    if removed_ids.is_empty() {
        return;
    }
    let tracked = REMOVAL_TRACKED_COMPS.lock();
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
