//! The standalone build: `cargo truce run -p meniscus`. Not shipped.

fn main() {
    truce_standalone::run::<meniscus::Plugin>();
}
