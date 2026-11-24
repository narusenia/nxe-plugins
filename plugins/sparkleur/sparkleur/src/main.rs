//! The standalone build: `cargo truce run -p sparkleur`. Not shipped.

fn main() {
    truce_standalone::run::<sparkleur::Plugin>();
}
