use bevy::prelude::*;

pub mod networking;

fn main() {
    let mut app = App::new();

    app.add_plugins(MinimalPlugins);

    networking::build(&mut app);

    app.run();
}
