/// Small, fast, seedable PRNG (mulberry32). The whole state is a single
/// 32-bit integer, so it can be saved with the game and restored exactly.
#[derive(Clone, Debug)]
pub struct Rng {
    pub state: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng { state: seed }
    }

    /// Seeded from the clock.
    pub fn from_time() -> Self {
        Rng::new(crate::platform::time_seed())
    }

    /// Returns a float in [0, 1).
    pub fn next(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6d2b79f5);
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        (t ^ (t >> 14)) as f64 / 4294967296.0
    }

    /// Returns an integer in [0, n).
    pub fn int(&mut self, n: usize) -> usize {
        (self.next() * n as f64).floor() as usize
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.int(items.len())]
    }

    /// Picks an item using the given non-negative weights.
    pub fn weighted<'a, T>(&mut self, items: &'a [T], weight: impl Fn(&T) -> f64) -> &'a T {
        let total: f64 = items.iter().map(&weight).sum();
        let mut roll = self.next() * total;
        for item in items {
            roll -= weight(item);
            if roll < 0.0 {
                return item;
            }
        }
        &items[items.len() - 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_javascript_mulberry32() {
        // Values produced by the original TypeScript implementation for seed 12345.
        let mut rng = Rng::new(12345);
        let a = rng.next();
        let b = rng.next();
        assert!((0.0..1.0).contains(&a) && (0.0..1.0).contains(&b));
        let mut again = Rng::new(12345);
        assert_eq!(again.next(), a);
        assert_eq!(again.next(), b);
        assert_eq!(Rng::new(1).next(), 0.6270739405881613);
    }

    #[test]
    fn weighted_respects_zero_weights() {
        let mut rng = Rng::new(7);
        for _ in 0..100 {
            assert_eq!(*rng.weighted(&[1, 2, 3], |&x| if x == 2 { 1.0 } else { 0.0 }), 2);
        }
    }
}
