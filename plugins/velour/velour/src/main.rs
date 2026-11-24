//! The standalone build: `cargo truce run -p velour`. Not shipped.

fn main() {
    truce_standalone::run::<velour::Plugin>();
}
