//! The standalone build: `cargo truce run -p invert`. Not shipped.

fn main() {
    truce_standalone::run::<invert::Plugin>();
}
