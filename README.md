# Avenix ECS Engine

A deterministic, concurrent Entity Component System (ECS) written in Rust, featuring high-performance parallel workloads, out-of-band remote coordination, native **States**, and advanced **SystemSets** grouping.

---

## Performance & Core Architecture

* **Archetype-Based Storage**  
  Built entirely around a strict structural Archetype architecture. Components are packed into flat, dense tables to maximize CPU cache-line saturation and optimize hardware prefetching loops.
* **Zero-UB Layout Execution**  
  Engineered with memory safety layout guarantees. Core pathways protect against undefined behavior without introducing overhead or sacrificing raw pointer iteration performance.
* **Multi-Threading**  
  Integrates a native Rayon worker pool to automatically chunk, partition, and process archetype arrays concurrently across all available CPU cores.
* **Race-Free Structural Isolation**  
  Prevents iterator invalidation by deferring entity mutations (spawning, component insertions, and despawning) into synchronized command buffers flushed exclusively at frame boundaries.
* **Advanced SystemSets Execution**  
  Groups systems into functional blocks that share operational bounds. Attaching a run condition to a SystemSet evaluates that condition **exactly once** for the entire group, removing the overhead of individual system evaluations.
* **Integrated States Pipeline**  
  Features a built-in Finite State Machine supporting edge-triggered transitions (`entered_state`, `exited_state`) and ongoing execution filters (`in_state`).

---

## Basic Example Usage

```rust
use avenix::prelude::*;

fn main() {
    App::new()
        .add_systems(Update, hello_world_system)
        .run();
}

fn hello_world_system() {
    println!("hello world");
}
```

---

## Parallel Handles

Avenix isolates out-of-band coordination into thread-safe, thread-clonable handles extracted directly from the application layer. Sourced via `app.world_mut()` (or pre-registered on `App` for queries), these handles act as detached remotes sent into background tasks, network loops, or worker threads to execute safely outside the main schedule thread.

```
                  ┌─────────────────────────────────────┐
                  │          Main App Thread            │
                  └──────────────────┬──────────────────┘
                                     │
                 Extract / Inject Parallel Handles
                                     │
          ┌──────────────────────────┼──────────────────────────┐
          ▼                          ▼                          ▼
┌──────────────────┐       ┌──────────────────┐       ┌──────────────────┐
│  Worker Thread   │       │  Network Thread  │       │  Async Task Pool │
│ ParallelCommands │       │ ParallelEvents   │       │ ParallelQueries  │
└──────────────────┘       └──────────────────┘       └──────────────────┘
```

### Configuration & Capabilities

* **`ParallelCommands`**
  - **Extraction:** Sourced using `world.get_par_commands()`.
  - **Capabilities:** Invoking `.scope(|cmd| ...)` grants access to a standard command buffer. This allows background threads to safely queue structural mutations (spawning/despawning entities, adding/removing components, and deferring resource swaps) to be flushed during the next apply phase.
* **`ParallelEventWriter<T>` & `ParallelEventReader<T>`**
  - **Extraction:** Sourced using `world.get_par_event_writer::<T>()` and `world.get_par_event_reader::<T>()`.
  - **Capabilities:** Invoking `.scope(|writer| ...)` or `.scope(|reader| ...)` lets concurrent background workers read, iterate, or push events synchronously across parallel tasks.
* **`ParallelResourceAccessor<T>`**
  - **Extraction:** Sourced using `world.get_par_resource_accessor::<T>()`.
  - **Capabilities:** Provides `.scope()`, `.scope_mut()`, `.scope_opt()`, and `.scope_mut_opt()` windows into global values. Supports direct structural management via `.insert_resource()` and `.remove_resource()`, and presence checks via `.is_present()`. Clone-safe.
* **`ParallelQueryAccessor<Q, F>`**
  - **Extraction:** Registered on the `App` layer using `app.get_par_query_accessor::<Q, F>()` **before** calling `app.build()`.
  - **Capabilities:** Opens read/write component data matrix structures (`Q: QueryData`) matching criteria filters (`F: QueryFilter`) for detached processing workers.
  
---

## Examples

Please refer to the `examples/` directory for full usage blueprints covering core commands, event streams, systems coordination, custom macro derives, and parallel processing layouts.

---

## Feature Flag Architecture

Avenix isolates core systems into optional build targets to keep untracked execution paths unburdened by default:

* `reactivity` — Activates double-buffered change tracking infrastructure (`Added<T>`, `Changed<T>`, `ChangedTracker<T>`). Uses demand-driven tracking queues allocated exclusively when systems invoke them.
* `events` — Initializes the global broadcasting event pipeline architectures (`EventWriter`, `EventReader`, and parallel variants).

---

## Lifecycle Constraints & Safety Invariants

### System Timing & The 3-Frame Buffer Rule
Avenix implements a deterministic, double-buffered tracking infrastructure for all component reactivity (`Added<T>`, `Changed<T>`) and system event broadcast layers. This mechanics operates on a strict, predictable **3-Frame Buffer Timeline** that ensures the system can run infinitely without memory spikes or drifting track footprints.

```
 [ Frame N: Staging ]      ➔    [ Frame N+1: Visible ]    ➔     [ Frame N+2: Reset ]
Mutations / Sends Occur          Buffers Swapped Globally        Mutation State Cleared
Hidden From Active Queries       Caught Natively By Readers      Space Reclaimed
```

* **Frame N (Staging Phase):** Component changes occur or events are broadcast. The internal trackers capture modifications but hide them from active loops to ensure intra-frame isolation and prevent cascading logic loops.
* **Frame N+1 (Visible Phase):** Double-buffered layouts swap automatically at the frame boundary. In-band systems, dynamic filters (`Changed<T>`, `Added<T>`, `RemovedComponents<T>`), and out-of-band parallel reader handles capture the aggregated dataset cleanly.
* **Frame N+2 (Reset Phase):** Generation metrics overwrite automatically. Track tokens and event spaces are instantly reclaimed by the engine, keeping structural allocation metrics perfectly flat across infinite execution loops.

---

### 32-Bit Generationless Entity Recyclability
Avenix enforces an explicit design invariant: **All cloned handles referencing a specific entity must be completely dropped before that entity's queued despawn command is processed.**

* **The Benefit:** By requiring clear lifecycle termination boundaries, Avenix completely eliminates the need for integer generation numbers or tracking ticks inside entity keys. The engine safely packs full entity addresses into a compact **32-bit slot**, minimizing memory consumption and eliminating integer overflow errors during long uptime operations.
* **Handling Cleanup:** The engine will invoke a diagnostic panic during the structural flush phase if a handle leak occurs, printing a detailed map of the component layout archetypes involved to help you isolate where the handle leak occurred. To satisfy this requirement cleanly, the engine provides dedicated stage lifecycles (`CleanupHandles`) and state-interception tools within the `Commands` to inspect scheduled despawns before the memory flush executes. Complete verification layouts can be observed directly within the `hierarchy_cleanup` example blueprint.

---

### Component Reactivity Architecture
When the `reactivity` feature is active, Avenix uses a demand-driven model to minimize runtime overhead.

* **Lazy Tracking Allocations:** Double-buffered tracking queues are not allocated for every component type by default. They are registered and initialized only if a system explicitly requests them (e.g., via `RemovedComponents<T>`).
* **Stripped Runtime Pathways:** Component types that are never used in reactive query filters skip frame-boundary memory swaps entirely, keeping untracked data paths unburdened by default.

---

## 📜 License

Avenix is dual-licensed under either:
* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
* MIT license ([LICENSE-MIT](LICENSE-MIT))
