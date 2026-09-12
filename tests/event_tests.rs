use avenix::prelude::*;
use rusty_fork::rusty_fork_test;
use std::thread;

#[derive(Event)]
struct ThreatEvent {
    value: f32,
}

#[derive(Resource)]
struct FrameCounter {
    current_frame: u32,
}

fn increment_frame_system(mut counter: ResMut<FrameCounter>) {
    counter.current_frame += 1;
}

fn pre_emit_verify_system(counter: Res<FrameCounter>, reader: EventReader<ThreatEvent>) {
    let count = reader.iter().count();
    assert!(
        reader.iter().all(|event| event.value == 10.0),
        "Value corrupted or not same"
    );
    if counter.current_frame == 1 {
        assert_eq!(
            count, 0,
            "❌ Soundness Failure: Pre-emit system found an event on Frame 1 before any emission occurred! Got: {}, Expected: 0",
            count
        );
    } else if counter.current_frame == 2 {
        assert_eq!(
            count, 1,
            "❌ Core Failure: Pre-emit system failed to pick up the historical inter-frame event on Frame 2! Got: {}, Expected: 1",
            count
        );
    } else if counter.current_frame == 3 {
        assert_eq!(
            count, 0,
            "❌ Stagnation Failure: Event leaked into Frame 3 for the Pre-emit system after it should have stagnated out of bounds! Got: {}, Expected: 0",
            count
        );
    }
}

fn emitter_system(counter: Res<FrameCounter>, mut writer: EventWriter<ThreatEvent>) {
    if counter.current_frame == 1 {
        writer.send(ThreatEvent { value: 10.0 });
    }
}

fn post_emit_verify_system(counter: Res<FrameCounter>, reader: EventReader<ThreatEvent>) {
    let count = reader.iter().count();
    assert!(
        reader.iter().all(|event| event.value == 10.0),
        "Value corrupted or not same"
    );
    if counter.current_frame == 1 {
        assert_eq!(
            count, 0,
            "❌ Buffer Break: Post-emit system caught the immediate inline event alteration on Frame 1! Got: {}, Expected: 0",
            count
        );
    } else if counter.current_frame == 2 {
        assert_eq!(
            count, 1,
            "❌ Pipeline Leak: Post-emit system failed to read the active buffered event on Frame 2! Got: {}, Expected: 1",
            count
        );
    } else if counter.current_frame == 3 {
        assert_eq!(
            count, 0,
            "❌ Stagnation Failure: Event leaked into Frame 3 for the Post-emit system after it should have stagnated out of bounds! Got: {}, Expected: 0",
            count
        );
    }
}

fn test_runner_frames(app: &mut App) {
    app.build();
    app.run_startup();

    app.update();
    app.update();
    app.update();
}

rusty_fork_test! {
    #[test]
    fn test_events_multi_frame_lifecycle() {
        let mut app = App::new();
        app.add_plugins(DefaultSchedulesPlugin)
            .insert_resource(FrameCounter { current_frame: 0 })
            .init_event::<ThreatEvent>()
            .add_systems(
                Update,
                (
                    increment_frame_system,
                    pre_emit_verify_system,
                    emitter_system,
                    post_emit_verify_system,
                ),
            );

        app.set_runner(test_runner_frames);
        app.run();
    }
}

fn parallel_execution_system(
    counter: Res<FrameCounter>,
    reader_a: EventReader<ThreatEvent>,
    reader_b: EventReader<ThreatEvent>,
    mut writer_a: EventWriter<ThreatEvent>,
    par_writer_a: ParallelEventWriter<ThreatEvent>,
    par_writer_b: ParallelEventWriter<ThreatEvent>,
    par_reader_a: ParallelEventReader<ThreatEvent>,
    par_reader_b: ParallelEventReader<ThreatEvent>,
) {
    if counter.current_frame == 1 {
        writer_a.send(ThreatEvent { value: 100.0 });

        let mut read_a_count = 0;
        let mut read_b_count = 0;
        let mut nest_read_a_count = 0;
        let mut nest_read_b_count = 0;

        std::thread::scope(|s| {
            s.spawn(|| {
                par_writer_a.scope(|mut writer| {
                    writer.send(ThreatEvent { value: 10.0 });

                    par_writer_b.scope(|mut writer| {
                        for _ in 0..4 {
                            writer.send(ThreatEvent { value: 10.0 });
                        }
                    });
                });
            });

            s.spawn(|| {
                par_writer_b.scope(|mut writer| {
                    writer.send(ThreatEvent { value: 10.0 });

                    par_writer_a.scope(|mut writer| {
                        for _ in 0..4 {
                            writer.send(ThreatEvent { value: 10.0 });
                        }
                    });
                });
            });

            s.spawn(|| {
                read_a_count = reader_a.iter().count();
                read_b_count = reader_b.iter().count();
            });

            s.spawn(|| {
                par_reader_a.scope(|reader| {
                    nest_read_a_count = reader.iter().count();

                    par_reader_b.scope(|reader| {
                        nest_read_b_count = reader.iter().count();
                    });
                });
            });
        });

        assert_eq!(read_a_count, 0);
        assert_eq!(read_b_count, 0);
        assert_eq!(nest_read_a_count, 0);
        assert_eq!(nest_read_b_count, 0);
    }
}

fn parallel_verification_system(
    counter: Res<FrameCounter>,
    reader_a: EventReader<ThreatEvent>,
    par_reader_a: ParallelEventReader<ThreatEvent>,
    par_reader_b: ParallelEventReader<ThreatEvent>,
) {
    let count = reader_a.iter().count();

    if counter.current_frame == 2 {
        assert_eq!(count, 11);

        let mut base_count = 0;
        let mut parallel_count = 0;
        let mut nest_read_a_count = 0;
        let mut nest_read_b_count = 0;

        for event in reader_a.iter() {
            if event.value == 100.0 {
                base_count += 1;
            } else if event.value == 10.0 {
                parallel_count += 1;
            }
        }

        assert_eq!(base_count, 1);
        assert_eq!(parallel_count, 10);

        thread::scope(|s| {
            s.spawn(|| {
                par_reader_a.scope(|reader_a| {
                    nest_read_a_count = reader_a.iter().count();

                    par_reader_b.scope(|reader_b| {
                        nest_read_b_count = reader_b.iter().count();
                    });
                });
            });
        });

        assert_eq!(
            nest_read_a_count, 11,
            "❌ Nested ParallelEventReader A missed historical events on Frame 2!"
        );
        assert_eq!(
            nest_read_b_count, 11,
            "❌ Nested ParallelEventReader B missed historical events on Frame 2!"
        );
    } else if counter.current_frame == 3 {
        assert_eq!(count, 0);
    }
}

fn test_two_frames(app: &mut App) {
    app.build();
    app.run_startup();

    app.update();
    app.update();
}

rusty_fork_test! {
    #[test]
    fn test_events_parallel_lifecycle() {
        let mut app = App::new();
        app.add_plugins(DefaultSchedulesPlugin)
            .insert_resource(FrameCounter { current_frame: 0 })
            .init_event::<ThreatEvent>()
            .add_systems(
                Update,
                (
                    increment_frame_system,
                    parallel_execution_system,
                    parallel_verification_system,
                ),
            );

        app.set_runner(test_two_frames);
        app.run();
    }
}
