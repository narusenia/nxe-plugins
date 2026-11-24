//! The standalone build: `cargo truce run -p buckle`. Not shipped.

fn main() {
    truce_standalone::run::<buckle::Plugin>();
}
