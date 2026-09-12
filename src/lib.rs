//! # Avenix Engine
//!
//! ## Core Architecture Invariants
//!
//! To maintain absolute safety and high performance, Avenix enforces strict operational rules:
//!
//! * **Entity Despawn Invariant:** All cloned handles referencing an entity must be completely dropped
//!   before that entity's scheduled despawn command is executed. Violating this triggers an explicit runtime
//!   panic displaying the archetype's components. See the [`CleanupHandles`] schedule to learn how its designed to
//!   help you with this, and read [`Commands::despawn()`] for more information.
//!
//! [`CleanupHandles`]: crate::schedule::CleanupHandles
//! [`Commands::despawn()`]: crate::commands::Commands::despawn

#[path = "app/mod.rs"]
mod app_impl;
mod commands;
mod component;
mod entity;
mod events;
mod query;
#[cfg(feature = "reactivity")]
mod reactivity;
mod registry;
mod resources;
mod schedule;
mod system;
mod world;

pub use fxhash;
pub use indexmap;
pub use rayon;

pub mod prelude {
    pub use crate::app_impl::{App, Plugin, PluginsBuildAll};
    pub use crate::commands::{Commands, ParallelCommands, bundle::ComponentBundle};
    pub use crate::ecs::derive::*;
    #[cfg(feature = "events")]
    pub use crate::ecs::events::*;
    pub use crate::entity::Entity;
    pub use crate::query::*;
    #[cfg(feature = "reactivity")]
    pub use crate::reactivity::*;
    pub use crate::resources::*;
    pub use crate::schedule::*;
    pub use crate::system::System;
}

pub mod extensions {
    pub use crate::system::system_storage::{FunctionData, SystemData, SystemExt};
    pub use crate::system::{
        AccessHashSet, AccessVec, FunctionSystem, IntoSystem, IntoSystemConfigs, System,
        SystemMeta, SystemParam,
    };
    pub use crate::world::archetypes::{Archetype, ComponentColumn};
    pub use crate::world::storage::World;
}

pub mod ecs {
    pub use crate::component::Component;
    pub mod derive {
        pub use avenix_macros::{
            Component, ComponentBundle, Event, QueryData, QueryFilter, Resource, SystemParam,
        };
    }
    #[cfg(feature = "events")]
    pub mod events {
        pub use crate::events::{
            Event, EventReader, EventWriter, ParallelEventReader, ParallelEventWriter,
        };
    }
    pub mod resources {
        pub use crate::resources::{Res, ResMut, Resource};
    }
    pub mod query {
        pub use crate::query::{Query, QueryArchetypeView, QueryData, QuerySubChunk};
        pub mod filter {
            pub use crate::query::filter::{
                EmptyQueryFilter, Not, Or, QueryFilter, StructuralQueryFilter, With, Without,
            };
        }
    }
    #[cfg(feature = "reactivity")]
    pub mod reactivity {
        pub mod changed {
            pub use crate::reactivity::{Changed, ChangedTracker};
        }
        pub mod added {
            pub use crate::reactivity::{Added, AddedTracker};
        }
        pub mod removed {
            pub use crate::reactivity::RemovedComponents;
        }
    }
    pub mod commands {
        pub use crate::commands::{Commands, DespawnCommand, ParallelCommands};
        pub mod bundle {
            pub use crate::commands::bundle::ComponentBundle;
        }
    }
    pub mod world {
        pub use crate::world::storage::World;
    }
}

pub mod app {
    pub use crate::app_impl::App;
    pub mod system {
        pub use crate::system::{
            AccessHashSet, AccessVec, FunctionSystem, IntoSystem, IntoSystemConfigs, System,
            SystemConfigs, SystemMeta, SystemParam,
            system_storage::{FunctionData, SystemData, SystemExt},
        };
    }
    pub mod schedule {
        pub use crate::schedule::{
            CleanupHandles, DefaultSchedulesPlugin, First, Last, PostUpdate, PreUpdate, Schedule,
            ScheduleLabel, Update,
        };
    }
    pub mod plugin {
        pub use crate::app_impl::{Plugin, PluginsBuildAll};
    }
}
