use avenix::prelude::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
enum GameplaySchedule {
    Update,
    PostUpdate,
}

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
struct SharedSet;

fn first_system() {}
fn second_system() {}

#[test_fork::test]
fn test_data_based_schedules() {
    let mut app = App::new();

    app.add_schedule(Schedule::new(GameplaySchedule::Update))
        .add_schedule(Schedule::new(GameplaySchedule::PostUpdate))
        .configure_schedule_order(GameplaySchedule::Update, GameplaySchedule::PostUpdate)
        .configure_sets(GameplaySchedule::Update, SharedSet)
        .add_systems(GameplaySchedule::Update, first_system.in_set(SharedSet))
        .add_systems(
            GameplaySchedule::PostUpdate,
            second_system.in_set(SharedSet),
        );

    let result = catch_unwind(AssertUnwindSafe(move || {
        app.run();
    }));

    assert!(result.is_err());
}
