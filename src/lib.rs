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

#![cfg_attr(docsrs, feature(doc_cfg))]

#[path = "app/mod.rs"]
mod app_impl;
mod commands;
mod component;
mod entity;
mod entity_registry;
mod events;
mod query;
#[cfg(feature = "reactivity")]
mod reactivity;
mod resources;
mod schedule;
mod system;
mod world;

pub mod derive {
    pub use avenix_macros::{
        Component, ComponentBundle, Event, QueryData, QueryFilter, Resource, SystemParam,
    };
}

pub use indexmap;
pub use orx_parallel;
pub use rayon;
pub use rustc_hash;

pub mod prelude {
    pub use crate::app::{
        App,
        plugin::{Plugin, PluginsBuildAll},
        schedule::{
            CleanupHandles, First, Last, PostUpdate, PreUpdate, Schedule, ScheduleLabel, Startup,
            Update,
        },
        system::{IntoSystem, IntoSystemConfigs, System, SystemConfigs, SystemMeta, SystemParam},
    };
    pub use crate::derive::{
        Component, ComponentBundle, Event, QueryData, QueryFilter, Resource, SystemParam,
    };
    #[cfg(feature = "events")]
    pub use crate::ecs::events::{
        Event, EventReader, EventWriter, ParallelEventReader, ParallelEventWriter,
    };
    #[cfg(feature = "reactivity")]
    pub use crate::ecs::reactivity::{
        added::{Added, AddedTracker},
        changed::{Changed, ChangedTracker},
        removed::RemovedComponents,
    };
    pub use crate::ecs::{
        Component,
        commands::{
            Commands, DespawnCommand, EntityCommands, ParallelCommands, bundle::ComponentBundle,
        },
        entity::Entity,
        query::{
            Has, Query, QueryArchetypeView, QueryData, QuerySubChunk,
            filter::{
                EmptyQueryFilter, Not, Or, QueryFilter, StructuralQueryFilter, With, Without,
            },
        },
        resources::{NonSend, NonSendMut, ParallelResourceAccessor, Res, ResMut, Resource},
        world::World,
    };
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
    #[cfg(feature = "events")]
    pub mod events {
        pub use crate::events::{
            Event, EventReader, EventWriter, ParallelEventReader, ParallelEventWriter,
        };
    }
    pub mod resources {
        pub use crate::resources::{
            NonSend, NonSendMut, ParallelResourceAccessor, Res, ResMut, Resource,
        };
    }
    pub mod query {
        pub use crate::query::{Has, Query, QueryArchetypeView, QueryData, QuerySubChunk};
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
        pub use crate::commands::{Commands, DespawnCommand, EntityCommands, ParallelCommands};
        pub mod bundle {
            pub use crate::commands::bundle::ComponentBundle;
        }
    }
    pub mod world {
        pub use crate::world::storage::World;
    }
    pub mod entity {
        pub use crate::entity::Entity;
    }
}

pub mod app {
    pub use crate::app_impl::App;
    pub mod system {
        pub use crate::system::{
            IntoSystem, IntoSystemConfigs, System, SystemConfigs, SystemMeta, SystemParam,
        };
    }
    pub mod schedule {
        pub use crate::schedule::{
            CleanupHandles, First, Last, PostUpdate, PreUpdate, Schedule, ScheduleLabel, Startup,
            SystemExecutor, Update,
        };
    }
    pub mod plugin {
        pub use crate::app_impl::{Plugin, PluginsBuildAll};
    }
}
