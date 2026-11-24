//! The standalone build: `cargo truce run -p peek`. Not shipped.

fn main() {
    truce_standalone::run::<peek::Plugin>();
}
