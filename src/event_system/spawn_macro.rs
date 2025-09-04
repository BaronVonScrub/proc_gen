#[macro_export]
macro_rules! spawn {
    ($commands:expr, $event:expr) => {
        $commands.queue(move |w: &mut bevy::prelude::World| {
            w.send_event($event);
        });
    };
}
