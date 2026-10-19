use avenix::prelude::*;

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
#[should_panic]
fn test_data_based_schedules() {
    App::new()
        .add_schedule(Schedule::new(GameplaySchedule::Update))
        .add_schedule(Schedule::new(GameplaySchedule::PostUpdate))
        .configure_schedule_order(GameplaySchedule::Update, GameplaySchedule::PostUpdate)
        .configure_sets(GameplaySchedule::Update, SharedSet)
        .add_systems(GameplaySchedule::Update, first_system.in_set(SharedSet))
        .add_systems(
            GameplaySchedule::PostUpdate,
            second_system.in_set(SharedSet),
        )
        .run();
}
