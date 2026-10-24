use avenix::prelude::*;

#[derive(Component)]
struct Position {
    #[allow(dead_code)]
    x: f32,
}

#[test_fork::test]
#[should_panic(expected = "SchedulesNotInitialized")]
fn test_panic_schedules_not_initialized() {
    let mut app = App::new();
    app.update();
}

#[test_fork::test]
#[should_panic(expected = "StartupNotProcessed")]
fn test_panic_startup_not_processed() {
    let mut app = App::new();
    app.build();
    app.update();
}

#[test_fork::test]
#[should_panic(expected = "AppAlreadyBuilt")]
fn test_panic_app_already_built() {
    let mut app = App::new();
    app.build();
    app.add_systems(Update, || {});
}

#[test_fork::test]
#[should_panic(expected = "AvenixLazyReactivityViolation")]
fn test_unregistered_changed_tracking_accessor_panics() {
    let mut app = App::new();
    app.build();
    let world = app.world();
    let _accessor = world.par_query_accessor::<&Position, Changed<Position>>();
}

#[test_fork::test]
fn test_registered_changed_tracking_accessor_does_not_panic() {
    let mut app = App::new();
    app.register_changed_trackers::<Position>();
    app.build();
    let world = app.world();
    let _accessor = world.par_query_accessor::<&Position, Changed<Position>>();
}

#[test_fork::test]
#[should_panic(expected = "AvenixMultipleAppInstancesError")]
fn test_multiple_app_instances_panic() {
    let _app1 = App::new();
    let _app2 = App::new();
}
