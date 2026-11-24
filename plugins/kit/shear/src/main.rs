//! The standalone build: `cargo truce run -p shear`. Not shipped.

fn main() {
    truce_standalone::run::<shear::Plugin>();
}
