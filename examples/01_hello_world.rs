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
