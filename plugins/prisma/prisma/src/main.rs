//! The standalone build: `cargo truce run -p prisma`. Not shipped.

fn main() {
    truce_standalone::run::<prisma::Plugin>();
}
