//! The standalone build: `cargo truce run -p pollen`. Not shipped.

fn main() {
    truce_standalone::run::<pollen::Plugin>();
}
