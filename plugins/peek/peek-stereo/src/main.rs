//! The standalone build: `cargo truce run -p peek-stereo`. Not shipped.

fn main() {
    truce_standalone::run::<peek_stereo::Plugin>();
}
