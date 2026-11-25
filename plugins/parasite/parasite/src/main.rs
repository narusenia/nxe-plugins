//! The standalone build: `cargo truce run -p parasite`. Not shipped.

fn main() {
    truce_standalone::run::<parasite::Plugin>();
}
