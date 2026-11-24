//! The standalone build: `cargo truce run -p peek-spectrogram`. Not shipped.

fn main() {
    truce_standalone::run::<peek_spectrogram::Plugin>();
}
