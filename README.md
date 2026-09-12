# Avenix ECS Engine

A deterministic, concurrent Entity Component System (ECS) written in Rust, featuring parallel workloads, and out-of-band coordination.

---

## Performance & Safety Guarantees

* **Miri-Validated Sandbox**  
  Built on raw pointer offsets and contiguous columns. Fully passes Miri verification with zero undefined behavior.
* **Contiguous Archetype Layout**  
  Uses Archetype layout for storing entities and their data.
* **Fork-Join Parallel Iteration**  
  Uses a Rayon worker pool to partition and stream archetype data chunks concurrently across multiple CPU cores.
* **Command Synchronization**  
  Structural changes (spawning, inserting, deleting) are buffered into a thread-safe queue and flushed at the end of each schedule run.
* **Thread-Independent Spawning**  
  You can extract command handles (`app.world_mut().get_par_commands()`) outside the main system loops. This allows background threads to safely queue asynchronous entity spawns.
* **Thread-Independent Event Broadcasting**  
  Background workers or network threads can pull standalone event handles (`get_par_event_writer::<T>()` / `get_par_event_reader::<T>()`) to broadcast or read global notifications without locking up the main loop.

---

## Example Usage

```rust
use avenix::prelude::*;

fn test_runner_once(app: &mut App) {
    app.build();
    app.run_startup();
    while app.world().get_resource::<FrameCounter>().current_frame != 10 {
        app.update();
    }
}

#[derive(Resource)]
struct FrameCounter {
    current_frame: u32,
}

fn main() {
    App::new()
        .add_plugins(DefaultSchedulesPlugin)
        .insert_resource(FrameCounter { current_frame: 0 })
        .add_systems(Update, hello_world_system)
        .set_runner(test_runner_once)
        .run();
}

fn hello_world_system(mut frame: ResMut<FrameCounter>) {
    frame.current_frame += 1;
    println!("hello world");
    println!("Current frame: {}", frame.current_frame);
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
Avenix enforces a strict handle count invariant to maintain memory safety across parallel schedules.

> [!IMPORTANT]
> All cloned handles referencing an entity must be completely dropped before that entity's queued despawn command is processed.

* **Deferred Execution:** Calling `commands.despawn(entity)` buffers the operation to be processed later during the command flush phase.
* **The Panic:** The engine will panic during command execution if any cloned handles for that target entity are still alive in memory.
* **The Diagnostic:** The panic message prints a `HashSet` containing the exact `std::any::type_name` of every component within that entity's archetype to help track down where the handle leak occurred.
* **The Resolution:** Review the `DefaultSchedulesPlugin` documentation to see how to use `despawn_iter` and `will_despawn` to clear handles before execution flushes.

---

## Feature & Module Matrix

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
Avenix keeps components, resources, and basic derives enabled by default. Scale performance by opting into optional compilation flags:

* `reactivity` – Activates double-buffered change tracking (`Added`, `Changed`, `RemovedComponents`).
* `events` – Activates the event broadcasting pipelines (`EventWriter`, `EventReader`, etc.).

### Component Reactivity Architecture
When the `reactivity` feature is active, Avenix uses a demand-driven model to minimize runtime overhead.

* **Lazy Tracking Allocations:** Double-buffered tracking queues are not allocated for every component type by default. They are registered and initialized only if a system explicitly requests them (e.g., via `RemovedComponents<T>`).
* **Stripped Runtime Pathways:** Component types that are never used in reactive query filters skip frame-boundary memory swaps entirely, keeping untracked data paths unburdened.

---

## Random Lookup Performance

The following data details random access lookup metrics using Criterion benchmarks. Target handle vectors are scrambled prior to execution to invalidate the CPU hardware prefetcher and force cache-line evictions.

### Test Hardware Profile
* **System:** Lenovo LOQ 15IAX9
* **Processor:** Intel Core i5 12th Gen
* **Operating System:** Linux

### Environment A: 100,000 Total Entities (Uniform Layout)
Measures baseline index routing speed in a clean world containing 100,000 uniform components.

* **4-Byte Component Payload (`query_lookups`)**
  * `query.get(entity)`: **1.16 ns** per lookup (116.58 µs total execution time)
  * `query.get_unchecked(entity)`: **0.75 ns** per lookup (75.91 µs total execution time)
* **256-Byte Heavy Payload (`query_lookups_heavy`)**
  * `query.get(entity)`: **1.17 ns** per lookup (117.49 µs total execution time)
  * `query.get_unchecked(entity)`: **0.76 ns** per lookup (76.31 µs total execution time)

### Environment B: 2,000,000 Total Entities (High Fragmentation)
Evaluates 100,000 target lookups scattered across a pool of 2,000,000 total background entities, fragmented across 5 distinct archetype tables to induce maximum cache line saturation pressure.

* **4-Byte Component Payload (`query_lookups_fragmented`)**
  * `query.get(entity)`: **1.64 ns** per lookup (164.04 µs total execution time)
  * `query.get_unchecked(entity)`: **1.15 ns** per lookup (115.51 µs total execution time)
* **256-Byte Heavy Payload (`query_lookups_fragmented_heavy`)**
  * `query.get(entity)`: **1.62 ns** per lookup (162.00 µs total execution time)
  * `query.get_unchecked(entity)`: **1.15 ns** per lookup (115.26 µs total execution time)

---

## Linear Iteration & Reactivity Performance

The following data evaluates linear iteration and mutation speeds over 10,000 entities under varying compilation flags and component states.

### Environment C: 10,000 Entities (Sequential Loop Pass)

* **Contiguous Memory Loop (`query_simple_iter`)**
  * Total execution time: **9.64 µs**
* **4-Archetype Split Memory Loop (`query_fragmented_iter`)**
  * Total execution time: **9.67 µs**
* **Pure Mutable Write - Reactivity Off (`query_write_pure`)**
  * Total execution time: **9.50 µs**
* **Reactive Feature On - Untracked Component (`query_write_reactive_untracked`)**
  * Total execution time: **11.36 µs**
* **Reactive Feature On - Tracked Component, Unfiltered System (`query_write_reactive_tracked`)**
  * `tracked_but_unfiltered_write`: **20.82 µs** total execution time
* **Reactive Feature On - Tracked Component, Filtered System (`query_write_reactive_tracked`)**
  * `tracked_and_filtered_write`: **1.03 ns** total execution time

---

## 📜 License

Avenix is dual-licensed under either:

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
* MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
