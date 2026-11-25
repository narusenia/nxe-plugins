//! The standalone build: `cargo truce run -p glint`. Not shipped.

fn main() {
    truce_standalone::run::<glint::Plugin>();
}
