//! The standalone build: `cargo truce run -p pumice`. Not shipped.

fn main() {
    truce_standalone::run::<pumice::Plugin>();
}
