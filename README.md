# Avenix ECS Engine

A deterministic, concurrent Entity Component System (ECS) written in Rust, featuring parallel workloads, and out-of-band coordination.

---
## Performance & Safety Architecture

* **Zero-UB Columnar Memory**  
  Engineered around low-level pointer layout optimization and contiguous memory lanes. Every core pathway fully passes strict Miri verification to guarantee complete runtime safety without sacrificing raw pointer performance.
* **Cache-Aligned Data Density**  
  Implements a strict Archetype structural layout. Components are packed into dense, flat tables to maximize CPU cache-line saturation and leverage hardware prefetching during heavy iteration loops.
* **Lock-Free Pipeline Concurrency**  
  Employs a native Rayon worker pool to automatically chunk, partition, and stream archetype tables across all available CPU cores, delivering seamless multi-threaded system execution.
* **Race-Free Structural Isolation**  
  Eliminates iterator invalidation and scheduling bottlenecks by deferring all entity mutations (spawning, component insertion, and despawning) into synchronized command buffers flushed strictly at frame boundaries.
* **Decoupled Out-of-Band Remotes**  
  Treats background tasks as first-class systems via thread-clonable parallel handles. External workers and network loops can safely manipulate resources and query entity matrix states asynchronously outside the main scheduling loop.
---

## Parallel Handles

Avenix provides a suite of thread-safe, thread-clonable handles extracted directly from the `World` layer (except for ParallelQueryAccessor). When extracting these handles from the application layer, pass through using `app.world_mut()`. Once obtained, these handles act as detached remotes that can be sent into background worker threads or external tasks to safely perform operations outside the main system scheduling loop.

### The Handles

* **`ParallelCommands`**
  * **How to get:** Call `world.get_par_commands()`.
  * **Usage:** Invoking `.scope(|mut cmd| ...)` grants access to a standard command buffer. This allows background threads to safely queue structural mutations (spawning/despawning entities, adding/removing components, and deferring resource swaps) to be flushed during the next apply phase.

* **`ParallelEventWriter<T>`**
  * **How to get:** Call `world.get_par_event_writer::<T>()`.
  * **Usage:** Invoking `.scope(|mut writer| ...)` allows out-of-band threads or network workers to push events into the shared event pipelines.
  * **Critical Constraints:** Subject to the engine's internal 3-frame buffering rule. Refer to the event system API docs for more information.

* **`ParallelEventReader<T>`**
  * **How to get:** Call `world.get_par_event_reader::<T>()`.
  * **Usage:** Invoking `.scope(|reader| ...)` lets concurrent background workers read and iterate over live event buffers synchronously.
  * **Critical Constraints:** Subject to the engine's internal 3-frame buffering rule. Refer to the event system API docs for more information.

* **`ParallelResourceAccessor<T>`**
  * **How to get:** Call `world.get_par_resource_accessor::<T>()`.
  * **Usage:** Invoking `.scope()`, `.scope_mut()`, `.scope_opt()`, or `.scope_mut_opt()` opens targeted closure windows into the resource registry. Features `.is_present()` for boolean presence checks, and allows instant registry adjustments using `.insert_resource()` and `.remove_resource()`.
  * **Critical Deadlock Warning:** Because this handle operates on fast, synchronous locks to maximize runtime throughput, invoking another scope with a mutable scope open on the exact same resource within the *same thread* will cause a deadlock. The framework bypasses runtime re-entrancy checks to preserve processing speed.

* **`ParallelQueryAccessor<Q, F>`**
  * **How to get:** Call `app.get_par_query_accessor::<Q, F>()` on the application layer *before* calling `app.build()`. 
  * **Usage:** Invoking `.scope(|query| ...)` spins up a localized `Query` matrix matching the requested component data structures (`Q: QueryData`) and criteria filters (`F: QueryFilter`). This gives background tasks raw, out-of-band iteration access over matching entity rows. It automatically configures and manages double-buffered component tracking columns based on your query filter signatures without any manual registration boilerplate.
  * **Critical Deadlock Warning:** Because this handle utilizes granular, column-level `RwLocks` to enable simultaneous multi-threaded table reading, nesting parallel query scopes incorrectly on the *same thread* will cause a deadlock. Specifically, opening a mutable query scope while an immutable or mutable query scope targeting overlapping components is already active within that thread will freeze execution.
---

## Example Usage

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

## Lifecycle Constraints

### The 1-Frame Visibility Rule
Avenix tracks data modifications through a double-buffered structural tracking network. 

> [!IMPORTANT]
> Any data adjustment evaluated via the `Changed<T>` or `Added<T>` filters remains visible to matching queries for a window of **exactly 1 execution frame**.

```text
 [ Frame N ]         ➔            [ Frame N+1 ]            ➔      [ Frame N+2 ]
Values Changed                    Double Buffers Swapped          Buffers Cleared
Trackers Update Interally         Visible to Queries              Tokens Overwritten
```

* **Frame N:** Values are changed. Internal trackers update but are hidden from active reads until the frame ends.
* **Frame N+1:** Structural buffers swap. Filtered queries intercept and read the changes.
* **Frame N+2:** Mutation tokens overwrite automatically. Visibility drops, and query states reset to normal.

*Note: All reactive logic using filters must run within this 1-frame boundary. Delaying system ticks past this window causes immediate mutation visibility decay.*

### The Entity Despawn Invariant
Avenix enforces a strict handle count invariant to maintain safety with recycled entity handles.

> [!IMPORTANT]
> All cloned handles referencing an entity must be completely dropped before that entity's queued despawn command is processed.

* **Deferred Execution:** Despawning an entity through commands buffers the operation to be processed later during the command flush phase.
* **The Panic:** The engine will panic during command execution if any cloned handles for that target entity are still alive in memory.
* **The Diagnostic:** The panic message prints a `HashSet` containing the exact `std::any::type_name` of every component within that entity's archetype to help track down where the handle leak occurred.
* **The Resolution:** Review the `CleanupHandles` schedule documentation to see how to use `despawn_iter` and `will_despawn` to clear handles before execution flushes.

---

## Feature & Module

### Required Procedural Macro Derives
Avenix requires explicit macro derives for core types to enforce static bounds checks and clean memory layouts. These respect standard visibility constraints (`pub`, `pub(crate)`):
* `#[derive(Component)]` – Marks a type as an archetype component.
* `#[derive(Resource)]` – Marks a type as a global unique resource.
* `#[derive(Event)]` – Marks a type as an event broadcast message.
* `#[derive(ComponentBundle)]` – Makes a bundle of components to make it type safe and easier to spawn entities.
* `#[derive(QueryData)]` – Makes a struct that can be used as QueryData to make it easier to code.
* `#[derive(QueryFilter)]` – Combines multiple conditional filters into a single struct.
* `#[derive(SystemParam)]` – Groups complex system parameters into a unified layout.

### Cargo Features
Avenix keeps components, resources, and basic derives enabled by default. You could opt into optional compilation flags:

* `reactivity` – Activates double-buffered change tracking (`Added`, `Changed`, `RemovedComponents`).
* `events` – Activates the event broadcasting pipelines (`EventWriter`, `EventReader`, etc.).

### Component Reactivity Architecture
When the `reactivity` feature is active, Avenix uses a demand-driven model to minimize runtime overhead.

* **Lazy Tracking Allocations:** Double-buffered tracking queues are not allocated for every component type by default. They are registered and initialized only if a system explicitly requests them (e.g., via `RemovedComponents<T>`).
* **Stripped Runtime Pathways:** Component types that are never used in reactive query filters skip frame-boundary memory swaps entirely, keeping untracked data paths unburdened.

---

## 📜 License

Avenix is dual-licensed under either:

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
* MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
