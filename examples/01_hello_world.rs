use avenix::prelude::*;

fn test_runner_once(app: &mut App) {
    app.build();
    app.run_startup();
    app.update();
}

fn main() {
    App::new()
        .add_systems(Update, hello_world_system)
        .set_runner(test_runner_once)
        .run();
}

fn hello_world_system() {
    println!("hello world");
}
