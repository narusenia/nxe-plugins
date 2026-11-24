//! The waveform, as a min / max pair per point, over a window the caller sets.
//!
//! **Min and max rather than one sample per point**, so a window much longer
//! than the points still shows the envelope instead of aliasing into noise.

pub struct Scope<const POINTS: usize> {
    mins: [f32; POINTS],
    maxs: [f32; POINTS],
    /// Where the next point goes. The oldest point is the one after it.
    next: usize,
    per_point: usize,
    filled: usize,
    low: f32,
    high: f32,
    sample_rate: f32,
}

impl<const POINTS: usize> Scope<POINTS> {
    pub fn new(sample_rate: f32) -> Self {
        let mut scope = Self {
            mins: [0.0; POINTS],
            maxs: [0.0; POINTS],
            next: 0,
            per_point: 1,
            filled: 0,
            low: f32::MAX,
            high: f32::MIN,
            sample_rate,
        };
        scope.set_window(0.05);
        scope
    }

    /// How much time the whole picture covers.
    pub fn set_window(&mut self, seconds: f32) {
        self.per_point = ((seconds * self.sample_rate / POINTS as f32) as usize).max(1);
    }

    pub fn push(&mut self, sample: f32) {
        let sample = if sample.is_finite() { sample } else { 0.0 };
        self.low = self.low.min(sample);
        self.high = self.high.max(sample);
        self.filled += 1;
        if self.filled >= self.per_point {
            self.mins[self.next] = self.low;
            self.maxs[self.next] = self.high;
            self.next = (self.next + 1) % POINTS;
            self.filled = 0;
            self.low = f32::MAX;
            self.high = f32::MIN;
        }
    }

    /// Oldest first.
    pub fn mins(&self) -> [f32; POINTS] {
        std::array::from_fn(|index| self.mins[(self.next + index) % POINTS])
    }

    pub fn maxs(&self) -> [f32; POINTS] {
        std::array::from_fn(|index| self.maxs[(self.next + index) % POINTS])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_point_is_last_and_keeps_the_extremes() {
        let mut scope: Scope<4> = Scope::new(400.0);
        scope.set_window(0.02); // two samples a point
        for sample in [0.1, -0.5, 0.9, 0.2] {
            scope.push(sample);
        }
        assert_eq!(scope.maxs()[2..], [0.1, 0.9]);
        assert_eq!(scope.mins()[2..], [-0.5, 0.2]);
    }
}
