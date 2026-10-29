use avenix::{app::schedule::MultiThreadedExecutor, prelude::*};
use std::sync::atomic::{AtomicUsize, Ordering};

static CAPTURED_THREADS: AtomicUsize = AtomicUsize::new(0);

fn create_custom_executor(label: &mut GameplaySchedule) -> MultiThreadedExecutor {
    if let GameplaySchedule::Update { threads } = label {
        CAPTURED_THREADS.store(*threads, Ordering::Relaxed);
    }
    MultiThreadedExecutor::new()
}

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
#[schedule_label(default_executor = create_custom_executor)]
enum GameplaySchedule {
    Update { threads: usize },
    PostUpdate,
}

#[derive(SystemSet, Clone, Debug, PartialEq, Eq)]
struct SharedSet;

fn first_system() {}
fn second_system() {}

#[test_fork::test]
#[should_panic(expected = "SystemSetError")]
fn test_data_based_schedules() {
    App::new()
        .add_schedule(Schedule::new(GameplaySchedule::Update { threads: 4 }))
        .add_schedule(Schedule::new(GameplaySchedule::PostUpdate))
        .configure_schedule_order(
            GameplaySchedule::Update { threads: 4 },
            GameplaySchedule::PostUpdate,
        )
        .configure_sets(GameplaySchedule::Update { threads: 4 }, SharedSet)
        .add_systems(
            GameplaySchedule::Update { threads: 4 },
            first_system.in_set(SharedSet),
        )
        .add_systems(
            GameplaySchedule::PostUpdate,
            second_system.in_set(SharedSet),
        )
        .run();
}

#[test_fork::test]
fn test_schedule_label_custom_executor_with_data() {
    CAPTURED_THREADS.store(0, Ordering::Relaxed);

    let mut label = GameplaySchedule::Update { threads: 8 };
    let _executor = label.default_executor();

    assert_eq!(CAPTURED_THREADS.load(Ordering::Relaxed), 8);
}
