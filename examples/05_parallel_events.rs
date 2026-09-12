use avenix::prelude::*;

#[derive(Event)]
struct ThreatEvent {
    value: f32,
}

#[derive(Resource)]
struct FrameCounter {
    current_frame: u32,
}

fn main() {
    println!("=== parallel event example ===\n");

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

    app.set_runner(test_three_frames);

    let par_writer_a = app.world_mut().get_par_event_writer::<ThreatEvent>();
    let par_writer_b = app.world_mut().get_par_event_writer::<ThreatEvent>();

    std::thread::scope(|s| {
        s.spawn(|| {
            par_writer_a.scope(|mut writer| {
                println!("[Thread 1] Sending parallel events...");
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
                println!("[Thread 2] Sending parallel events...");
                writer.send(ThreatEvent { value: 10.0 });
                par_writer_a.scope(|mut writer| {
                    for _ in 0..4 {
                        writer.send(ThreatEvent { value: 10.0 });
                    }
                });
            });
        });
    });

    app.run();

    println!("\n=== Parallel events example complete ===");
}

fn increment_frame_system(mut tracker: ResMut<FrameCounter>) {
    tracker.current_frame += 1;
}

fn parallel_execution_system(
    counter: Res<FrameCounter>,
    reader: EventReader<ThreatEvent>,
    mut writer: EventWriter<ThreatEvent>,
    par_writer_a: ParallelEventWriter<ThreatEvent>,
    par_writer_b: ParallelEventWriter<ThreatEvent>,
    par_reader_a: ParallelEventReader<ThreatEvent>,
    par_reader_b: ParallelEventReader<ThreatEvent>,
) {
    if counter.current_frame == 1 {
        println!("[System Frame 1] Sending standard baseline event...");
        writer.send(ThreatEvent { value: 100.0 });

        let mut inline_read_count = 0;
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
                inline_read_count = reader.iter().count();
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

        assert_eq!(inline_read_count, 0);
        assert_eq!(nest_read_a_count, 0);
        assert_eq!(nest_read_b_count, 0);
    }
}

fn parallel_verification_system(
    counter: Res<FrameCounter>,
    reader: EventReader<ThreatEvent>,
    par_reader_a: ParallelEventReader<ThreatEvent>,
    par_reader_b: ParallelEventReader<ThreatEvent>,
) {
    let count = reader.iter().count();

    if counter.current_frame == 2 {
        let mut base_count = 0;
        let mut parallel_count = 0;
        let mut nest_read_a_count = 0;
        let mut nest_read_b_count = 0;

        for event in reader.iter() {
            if event.value == 100.0 {
                base_count += 1;
            } else if event.value == 10.0 {
                parallel_count += 1;
            }
        }

        std::thread::scope(|s| {
            s.spawn(|| {
                par_reader_a.scope(|reader| {
                    nest_read_a_count = reader.iter().count();
                    par_reader_b.scope(|reader| {
                        nest_read_b_count = reader.iter().count();
                    });
                });
            });
        });

        println!("\n>> Avenix Parallel Event Analysis (Frame 2):");
        println!("   -> Total events collected across threads: {}", count);
        println!(
            "   -> Parallel Reader A verified historic count: {}",
            nest_read_a_count
        );
        println!(
            "   -> Parallel Reader B verified historic count: {}",
            nest_read_b_count
        );

        assert_eq!(base_count, 1);
        assert_eq!(parallel_count, 20);
        assert_eq!(count, 21);
        assert_eq!(nest_read_a_count, 21);
        assert_eq!(nest_read_b_count, 21);
        println!("   -> Success: All 21 concurrent events aggregated and synchronized perfectly!");
    } else if counter.current_frame == 3 {
        println!("\n>> Avenix Parallel Event Analysis (Frame 3):");
        println!(
            "   -> Checking parallel buffer decay. Reader count: {}",
            count
        );
        assert_eq!(count, 0);
        println!("   -> Success: Parallel event buffers automatically decayed and cleared!");
    }
}

fn test_three_frames(app: &mut App) {
    app.build();
    app.run_startup();

    app.update();
    app.update();
    app.update();
}
