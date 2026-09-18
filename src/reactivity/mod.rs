#![cfg(feature = "reactivity")]

use crate::extensions::ComponentColumn;
use indexmap::IndexMap;
use parking_lot::RwLock;
use rustc_hash::FxBuildHasher;
use std::any::{Any, TypeId};

mod added;
mod changed;
mod removed;

pub(crate) struct TrackedComponentMeta {
    pub(crate) component_id: TypeId,
    pub(crate) marker_id: TypeId,
    pub(crate) create_marker_column: fn() -> ComponentColumn,
    pub(crate) push_default_marker: unsafe fn(&mut ComponentColumn),
    pub(crate) clear_column_markers: unsafe fn(&mut dyn Any),
}

pub(crate) static TRACKED_COMPONENTS: RwLock<
    IndexMap<TypeId, TrackedComponentMeta, FxBuildHasher>,
> = RwLock::new(IndexMap::with_hasher(FxBuildHasher));

pub use changed::{Changed, ChangedTracker};

pub(crate) use changed::{ChangedMarker, Mut};

pub use added::{Added, AddedTracker};

pub use removed::RemovedComponents;

pub(crate) use removed::{REMOVAL_TRACKED_COMPS, register_removal_tracking_buffers};
