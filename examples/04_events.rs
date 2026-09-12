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
    println!("=== Standard avenix event lifecycle ===\n");

    let mut app = App::new();
    app.add_plugins(DefaultSchedulesPlugin)
        .insert_resource(FrameCounter { current_frame: 0 })
        .init_event::<ThreatEvent>()
        .add_systems(
            Update,
            (
                increment_frame_system,
                event_execution_system,
                event_verification_system,
            ),
        );

    app.set_runner(test_three_frames);
    app.run();

    println!("\n=== Event verification completed ===");
}

fn increment_frame_system(mut tracker: ResMut<FrameCounter>) {
    tracker.current_frame += 1;
}

fn event_execution_system(
    counter: Res<FrameCounter>,
    reader: EventReader<ThreatEvent>,
    mut writer: EventWriter<ThreatEvent>,
) {
    if counter.current_frame == 1 {
        println!("[Frame 1] Sending events via EventWriter...");
        writer.send(ThreatEvent { value: 100.0 });
        writer.send(ThreatEvent { value: 50.0 });
        let instant_read_count = reader.iter().count();
        assert_eq!(instant_read_count, 0);
    }
}

fn event_verification_system(counter: Res<FrameCounter>, reader: EventReader<ThreatEvent>) {
    let count = reader.iter().count();

    if counter.current_frame == 2 {
        println!("\n>> Avenix Event Analysis (Frame 2):");
        println!("   -> EventReader verified count: {}", count);
        assert_eq!(count, 2);

        let mut values = Vec::new();
        for event in reader.iter() {
            values.push(event.value);
        }
        assert!(values.contains(&100.0));
        assert!(values.contains(&50.0));

        println!("   -> Success: All events synchronized cleanly on Frame 2!");
    } else if counter.current_frame == 3 {
        println!("\n>> Avenix Event Analysis (Frame 3):");
        println!("   -> Checking event buffer decay. Reader count: {}", count);
        assert_eq!(count, 0);
        println!("   -> Success: Event buffers automatically decayed and cleared!");
    }
}

fn test_three_frames(app: &mut App) {
    app.build();
    app.run_startup();

    app.update();
    app.update();
    app.update();
}
