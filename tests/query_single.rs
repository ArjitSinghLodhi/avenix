use avenix::prelude::*;

#[derive(Component)]
struct TargetComponent(u32);

#[derive(Component)]
struct FilterComponent;

#[derive(Resource, Default)]
struct RunTracker {
    single_read_count: u32,
    single_mut_count: u32,
    skipped_count: u32,
}

fn system_with_single_read(
    single_target: Single<&TargetComponent>,
    mut tracker: ResMut<RunTracker>,
) {
    if single_target.0 == 42 {
        tracker.single_read_count += 1;
    }
}

fn system_with_single_mut(
    mut single_target: SingleMut<&mut TargetComponent>,
    mut tracker: ResMut<RunTracker>,
) {
    single_target.0 += 1;
    tracker.single_mut_count += 1;
}

fn system_with_single_filter(
    single_target: Single<&TargetComponent, With<FilterComponent>>,
    mut tracker: ResMut<RunTracker>,
) {
    if single_target.0 == 100 {
        tracker.single_read_count += 1;
    }
}

fn fallback_system(mut tracker: ResMut<RunTracker>) {
    tracker.skipped_count += 1;
}

fn verify_mutated_value_system(target: Single<&TargetComponent>, mut tracker: ResMut<RunTracker>) {
    if target.0 == 11 {
        tracker.skipped_count = 999;
    }
}

#[test_fork::test]
fn test_single_param_nominal_execution() {
    let mut app = App::new();
    app.insert_resource(RunTracker::default());
    app.add_systems(Startup, |commands: Commands| {
        commands.spawn((TargetComponent(42), FilterComponent));
    });
    app.add_systems(Update, (system_with_single_read, system_with_single_filter));

    app.build();
    app.run_startup();
    app.update();

    let tracker = app.world().resource::<RunTracker>();
    assert_eq!(tracker.single_read_count, 1);
    assert_eq!(tracker.skipped_count, 0);
}

#[test_fork::test]
fn test_single_param_skips_when_missing() {
    let mut app = App::new();
    app.insert_resource(RunTracker::default());
    app.add_systems(Update, (system_with_single_read, fallback_system).chain());

    app.build();
    app.run_startup();
    app.update();

    let tracker = app.world().resource::<RunTracker>();
    assert_eq!(tracker.skipped_count, 1);
    assert_eq!(tracker.single_read_count, 0);
}

#[test_fork::test]
fn test_single_param_skips_when_multiple_entities() {
    let mut app = App::new();
    app.insert_resource(RunTracker::default());
    app.add_systems(Startup, |commands: Commands| {
        commands.spawn(TargetComponent(42));
        commands.spawn(TargetComponent(42));
    });
    app.add_systems(Update, (system_with_single_read, fallback_system).chain());

    app.build();
    app.run_startup();
    app.update();

    let tracker = app.world().resource::<RunTracker>();
    assert_eq!(tracker.skipped_count, 1);
    assert_eq!(tracker.single_read_count, 0);
}

#[test_fork::test]
fn test_single_mut_nominal_and_modification() {
    let mut app = App::new();
    app.insert_resource(RunTracker::default());
    app.add_systems(Startup, |commands: Commands| {
        commands.spawn(TargetComponent(10));
    });
    app.add_systems(
        Update,
        (system_with_single_mut, verify_mutated_value_system).chain(),
    );

    app.build();
    app.run_startup();
    app.update();

    let tracker = app.world().resource::<RunTracker>();
    assert_eq!(tracker.single_mut_count, 1);
    assert_eq!(tracker.skipped_count, 999);
}

#[test_fork::test]
fn test_single_param_skips_when_filter_fails() {
    let mut app = App::new();
    app.insert_resource(RunTracker::default());
    app.add_systems(Startup, |commands: Commands| {
        commands.spawn(TargetComponent(100));
    });
    app.add_systems(Update, (system_with_single_filter, fallback_system).chain());

    app.build();
    app.run_startup();
    app.update();

    let tracker = app.world().resource::<RunTracker>();
    assert_eq!(tracker.skipped_count, 1);
    assert_eq!(tracker.single_read_count, 0);
}
