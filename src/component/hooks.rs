use parking_lot::RwLock;
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::{
    any::{TypeId, type_name},
    marker::PhantomData,
};

use crate::{
    commands::bundle::ComponentBundle,
    component::{ComponentMut, ComponentRef},
    ecs::component::Component,
    entity::Entity,
    events::{Event, EventReader, EventWriter},
    resources::{NonSend, NonSendMut, Res, ResMut, Resource},
    world::World,
};

pub type ComponentHook = fn(&mut ExclusiveWorld, &Entity);

#[derive(Clone, Default, Copy)]
pub(crate) struct ComponentHooks {
    pub(crate) on_add: Option<ComponentHook>,
    pub(crate) on_insert: Option<ComponentHook>,
    pub(crate) on_remove: Option<ComponentHook>,
    pub(crate) on_discard: Option<ComponentHook>,
}

pub struct ExclusiveWorld<'a> {
    pub(crate) world: &'a mut World,
}

impl<'a> ExclusiveWorld<'a> {
    pub fn get<'w, C: Component>(&'w self, entity: &'w Entity) -> Option<ComponentRef<'w, C>> {
        self.world.get::<C>(entity)
    }

    pub fn get_mut<'w, C: Component>(
        &'w mut self,
        entity: &'w Entity,
    ) -> Option<ComponentMut<'w, C>> {
        self.world.get_mut::<C>(entity)
    }

    pub fn spawn<B: ComponentBundle>(&mut self, component_bundle: B) {
        self.world.spawn(component_bundle);
    }

    pub fn spawn_batch<B, I>(&mut self, components_iter: I)
    where
        B: ComponentBundle,
        I: Iterator<Item = B> + Send + 'static,
    {
        self.world.spawn_batch(components_iter);
    }

    pub fn insert_components<B: ComponentBundle>(&mut self, entity: Entity, component_bundle: B) {
        self.world.insert_components(entity, component_bundle);
    }

    pub fn remove_components<B: ComponentBundle>(&mut self, entity: Entity) {
        self.world.remove_components::<B>(entity);
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.world.despawn(entity)
    }

    pub fn resource<'w, R: Resource + Send + Sync>(&'w self) -> Res<'w, R> {
        self.world.resource::<R>()
    }

    pub fn resource_mut<'w, R: Resource + Send + Sync>(&'w self) -> ResMut<'w, R> {
        self.world.resource_mut::<R>()
    }

    pub fn get_resource<'w, R: Resource + Send + Sync>(&'w self) -> Option<Res<'w, R>> {
        self.world.get_resource::<R>()
    }

    pub fn get_resource_mut<'w, R: Resource + Send + Sync>(&'w self) -> Option<ResMut<'w, R>> {
        self.world.get_resource_mut::<R>()
    }

    pub fn non_send_resource<'w, R: Resource>(&'w self) -> NonSend<'w, R> {
        self.world.non_send_resource::<R>()
    }

    pub fn non_send_resource_mut<'w, R: Resource>(&'w mut self) -> NonSendMut<'w, R> {
        self.world.non_send_resource_mut::<R>()
    }

    pub fn get_non_send_resource<'w, R: Resource>(&'w self) -> Option<NonSend<'w, R>> {
        self.world.get_non_send_resource::<R>()
    }

    pub fn get_non_send_resource_mut<'w, R: Resource>(&'w mut self) -> Option<NonSendMut<'w, R>> {
        self.world.get_non_send_resource_mut::<R>()
    }

    /// Retrieve a mutable reference to a non-send resource using a shared reference.
    ///
    /// # Safety
    /// The caller must ensure that no other mutable references to the same `NonSend`
    /// resource are active at the same time. Aliasing a mutable reference triggers
    /// immediate undefined behavior.
    pub unsafe fn non_send_resource_mut_unsafe<'w, R: Resource>(&'w self) -> NonSendMut<'w, R> {
        unsafe { self.world.non_send_resource_mut_unsafe::<R>() }
    }

    /// Safely attempt to retrieve a mutable reference to a non-send resource using a shared reference.
    /// Returns `None` if the resource does not exist.
    ///
    /// # Safety
    /// The caller must ensure that no other mutable references to the same `NonSend`
    /// resource are active at the same time.
    pub unsafe fn get_non_send_resource_mut_unsafe<'w, R: Resource>(
        &'w self,
    ) -> Option<NonSendMut<'w, R>> {
        unsafe { self.world.get_non_send_resource_mut_unsafe::<R>() }
    }

    pub fn event_writer<'w, E: Event>(&'w self) -> EventWriter<'w, E> {
        self.world.event_writer()
    }
    pub fn event_reader<'w, E: Event>(&'w self) -> EventReader<'w, E> {
        self.world.event_reader()
    }
}

pub(crate) struct HookRegistry {
    hooks: FxHashMap<TypeId, ComponentHooks>,
}

impl HookRegistry {
    pub(crate) const fn new() -> Self {
        Self {
            hooks: FxHashMap::with_hasher(FxBuildHasher),
        }
    }

    pub(crate) fn register_hooks_or_panic<T: Component>(&mut self) {
        let type_id = TypeId::of::<T>();

        if self.hooks.contains_key(&type_id) {
            panic!(
                "AvenixComponentHooksError\n\
                Component hooks for {:?} have already been defined! \
                Overwriting or multi-defining hook entries is strictly disallowed.",
                type_name::<T>()
            );
        }

        self.hooks.insert(type_id, ComponentHooks::default());
    }

    pub(crate) fn get_hooks_or_init<C: Component>(&mut self) -> ComponentHooks {
        let type_id = TypeId::of::<C>();
        if let Some(hooks) = self.hooks.get(&type_id) {
            return *hooks;
        }

        *self.hooks.entry(type_id).or_insert_with(|| ComponentHooks {
            on_add: C::on_add(),
            on_insert: C::on_insert(),
            on_remove: C::on_remove(),
            on_discard: C::on_discard(),
        })
    }

    pub(crate) fn get_hooks(&self, type_id: TypeId) -> Option<ComponentHooks> {
        self.hooks.get(&type_id).copied()
    }
}

pub(crate) static GLOBAL_HOOK_REGISTRY: RwLock<HookRegistry> = RwLock::new(HookRegistry::new());

pub struct ComponentHooksRegisterer<'a, T: Component> {
    _marker: PhantomData<(&'a mut T, &'a mut World)>,
}

impl<'a, T: Component> ComponentHooksRegisterer<'a, T> {
    pub(crate) fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    pub fn on_add(&self, hook: ComponentHook) -> &Self {
        self.try_on_add(hook)
            .expect("Component already has an on_add hook")
    }

    pub fn on_insert(&self, hook: ComponentHook) -> &Self {
        self.try_on_insert(hook)
            .expect("Component already has an on_insert hook")
    }

    pub fn on_discard(&self, hook: ComponentHook) -> &Self {
        self.try_on_discard(hook)
            .expect("Component already has an on_discard hook")
    }

    pub fn on_remove(&self, hook: ComponentHook) -> &Self {
        self.try_on_remove(hook)
            .expect("Component already has an on_remove hook")
    }

    pub fn try_on_add(&self, hook: ComponentHook) -> Option<&Self> {
        let mut registry = GLOBAL_HOOK_REGISTRY.write();
        let entry = registry.hooks.get_mut(&TypeId::of::<T>()).unwrap();
        if entry.on_add.is_some() {
            return None;
        }
        entry.on_add = Some(hook);
        Some(self)
    }

    pub fn try_on_insert(&self, hook: ComponentHook) -> Option<&Self> {
        let mut registry = GLOBAL_HOOK_REGISTRY.write();
        let entry = registry.hooks.get_mut(&TypeId::of::<T>()).unwrap();
        if entry.on_insert.is_some() {
            return None;
        }
        entry.on_insert = Some(hook);
        Some(self)
    }

    pub fn try_on_discard(&self, hook: ComponentHook) -> Option<&Self> {
        let mut registry = GLOBAL_HOOK_REGISTRY.write();
        let entry = registry.hooks.get_mut(&TypeId::of::<T>()).unwrap();
        if entry.on_discard.is_some() {
            return None;
        }
        entry.on_discard = Some(hook);
        Some(self)
    }

    pub fn try_on_remove(&self, hook: ComponentHook) -> Option<&Self> {
        let mut registry = GLOBAL_HOOK_REGISTRY.write();
        let entry = registry.hooks.get_mut(&TypeId::of::<T>()).unwrap();
        if entry.on_remove.is_some() {
            return None;
        }
        entry.on_remove = Some(hook);
        Some(self)
    }
}
