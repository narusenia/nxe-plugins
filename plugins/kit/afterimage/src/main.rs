//! The standalone build: `cargo truce run -p afterimage`. Not shipped.

fn main() {
    truce_standalone::run::<afterimage::Plugin>();
}
