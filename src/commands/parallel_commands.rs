use crate::commands::Commands;
use crate::commands::command_queue::CommandQueue;
use crate::commands::concurrent_despawn_set::ConcurrentBitSet;
use crate::system::{ParallelSystemParam, SystemMeta, SystemParam};
use crate::world::World;
use parking_lot::RwLock;
use std::sync::Arc;

/// A thread-safe, thread-clonable handle that acts as a detached remote input into the ECS engine.
///
/// `ParallelCommands` can be passed to external or background worker threads, allowing them to
/// concurrently queue up structural modifications and despawn targets outside the main execution
/// path. It can be obtained directly from the world layer via [`.par_commands()`] using `app.world().par_commands()`.
///
/// [`.par_commands()`]: crate::world::World::par_commands
pub struct ParallelCommands {
    pub(crate) queue: Arc<RwLock<CommandQueue>>,
    pub(crate) despawns: Arc<ConcurrentBitSet>,
}

impl Clone for ParallelCommands {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            despawns: self.despawns.clone(),
        }
    }
}

impl ParallelCommands {
    /// Creates a temporary [`Commands`] context bound to the current scope block.
    ///
    /// This allows external or parallel tasks to safely issue commands using the engine's
    /// standard command syntax via an inner closure.
    pub fn scope<F, R>(&self, f: F) -> R
    where
        F: for<'b> FnOnce(Commands<'b>) -> R,
    {
        let commands = Commands {
            queue: self.queue.read(),
            despawns: self.despawns.clone(),
        };
        f(commands)
    }
}

unsafe impl Send for ParallelCommands {}
unsafe impl Sync for ParallelCommands {}
unsafe impl ParallelSystemParam for ParallelCommands {}

impl SystemParam for ParallelCommands {
    type Item<'world> = ParallelCommands;

    fn init_access(_system_meta: &mut SystemMeta) {}

    fn get_param<'world>(world: &'world World) -> Option<Self::Item<'world>> {
        Some(ParallelCommands {
            queue: world.commands.queue.clone(),
            despawns: world.commands.despawns.clone(),
        })
    }
}
