pub(crate) mod hooks;

use std::ops::{Deref, DerefMut};

use crate::{
    component::hooks::ComponentHook,
    reactivity::changed::{ChangedMarker, Mut},
    world::archetypes::{ComponentColumnRead, ComponentColumnWrite},
};

pub trait Component: Send + Sync + 'static {
    /// Callback executed synchronously when this component is newly added to an entity.
    ///
    /// ### Execution Windows
    /// * **`SpawnCommand` & `BatchSpawnCommand`**: Fires for every component inside the bundle.
    /// * **`InsertComponentsCommand`**: Fires **ONLY if the component type was NOT already present** on the entity before execution.
    ///
    /// ### Framework Guarantees & State
    /// * **Storage State**: The entity has been completely migrated to its target archetype destination. All structural allocations are fully complete.
    /// * **Component State**: The newly provided data payload is fully written into the table column and is active.
    /// * **Sibling Components**: If introduced as part of a bundle or tuple, all sibling columns are fully populated and safely readable.
    /// * **Context Safety**: Archetype write locks are fully dropped before entry, making it safe to use immediate structural operations on the `DeferredWorld` context.
    fn on_add() -> Option<ComponentHook> {
        None
    }

    /// Callback executed synchronously every time this component is explicitly written or updated via a command gateway.
    ///
    /// ### Execution Windows
    /// * **`SpawnCommand` & `BatchSpawnCommand`**: Fires for every component inside the bundle immediately after `on_add` finishes execution.
    /// * **`InsertComponentsCommand`**: **ALWAYS fires**, regardless of whether the component type is brand new (`old_arch_id != new_arch_id`) or an in-place value overwrite (`old_arch_id == new_arch_id`).
    ///
    /// ### Framework Guarantees & State
    /// * **Ordering**: Always executes *after* any matching `on_add` hook (if the component type was newly introduced) or *after* an `on_discard` hook (if it overwrote existing data).
    /// * **Storage State**: Structural layout migrations are finalized and table indices are locked down in the entity registry cell.
    /// * **Component State**: Reflects the absolute latest incoming data payload state inside the table row.
    fn on_insert() -> Option<ComponentHook> {
        None
    }

    /// Callback executed synchronously when this component type is removed from an entity or dropped during a despawn operation.
    ///
    /// ### Execution Windows
    /// * **`RemoveComponentsCommand`**: Fires for all explicitly targeted component types before column data erasure occurs.
    /// * **`DespawnCommand`**: Fires for **every single component type currently present** on the entity before the entity index row is wiped out.
    ///
    /// ### Framework Guarantees & State
    /// * **Storage State**: The target entity is still located at its original archetype location while the hooks execute.
    /// * **Component State**: The component data payload is 100% valid and readable. The hook intercept runs right before the memory block is dropped or swapped.
    /// * **Sibling Components**: All other components on the entity remain intact and readable throughout the scope of this hook callback.
    /// * **Entity Lifecycle Coordination**: This hook fires **before** the engine executes the 32-Bit Generationless Entity Recyclability check.
    ///   Because all sibling components remain intact and readable during this window, developers can use this callback to drop, clear, or
    ///   unregister lingering cloned entity handles stored inside this component, **or inside any other sibling components** that this
    ///   component has knowledge of, safely satisfying the zero-leak invariant before the structural flush phase finishes and prevents
    ///   a diagnostic cleanup panic.
    fn on_remove() -> Option<ComponentHook> {
        None
    }

    /// Callback executed synchronously right before an existing component's data payload is overwritten by a new value.
    ///
    /// ### Execution Windows
    /// * **`InsertComponentsCommand`**: Fires **ONLY if the component type was ALREADY present** on the entity before the command was applied.
    ///   * *Case A (In-Place Overwrite)*: Fires right before the new value is swapped into the current row index.
    ///   * *Case B (Structural Shift)*: Fires if the component was present in the old archetype and is also present in the incoming bundle.
    ///
    /// ### Framework Guarantees & State
    /// * **Storage State**: Executed safely outside structural table lock scopes.
    /// * **Component State**: Reflects the PREVIOUS generation data payload. This provides a clean intercept window to read or cache old state data right before it is overwritten and dropped.
    /// * **Sibling Components**: All sibling data fields matching the current row remain fully accessible.
    fn on_discard() -> Option<ComponentHook> {
        None
    }
}

pub struct ComponentRef<'a, T: Component> {
    pub(crate) _guard: ComponentColumnRead<'a, T>,
    pub(crate) ptr: *const T,
}

impl<'a, T: Component> Deref for ComponentRef<'a, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

pub struct ComponentMut<'a, T: Component> {
    pub(crate) _guard_val: ComponentColumnWrite<'a, T>,
    pub(crate) _guard_marker: Option<ComponentColumnWrite<'a, ChangedMarker<T>>>,
    pub(crate) item: Mut<'a, T>,
}

impl<'a, T: Component> Deref for ComponentMut<'a, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<'a, T: Component> DerefMut for ComponentMut<'a, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.item
    }
}
