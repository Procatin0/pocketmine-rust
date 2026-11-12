//! PocketMine-MP XorShift128 Random Number Generator port.
//!
//! Matches `pocketmine\utils\Random`.

#[derive(Debug, Clone)]
pub struct Random {
    seed: i64,
    x: i64,
    y: i64,
    z: i64,
    w: i64,
}

impl Random {
    pub const X: i64 = 123456789;
    pub const Y: i64 = 362436069;
    pub const Z: i64 = 521288629;
    pub const W: i64 = 88675123;

    pub fn new(seed: i64) -> Self {
        let mut r = Self {
            seed: 0,
            x: 0,
            y: 0,
            z: 0,
            w: 0,
        };
        r.set_seed(seed);
        r
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = seed;
        self.x = Self::X ^ seed;
        self.y = (Self::Y ^ (seed << 17)) | (((seed >> 15) & 0x7fffffff) & 0xffffffff);
        self.z = (Self::Z ^ (seed << 31)) | (((seed >> 1) & 0x7fffffff) & 0xffffffff);
        self.w = (Self::W ^ (seed << 18)) | (((seed >> 14) & 0x7fffffff) & 0xffffffff);
    }

    pub fn get_seed(&self) -> i64 {
        self.seed
    }

    pub fn next_signed_int(&mut self) -> i32 {
        let t = (self.x ^ (self.x << 11)) & 0xffffffff;
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        self.w =
            (self.w ^ ((self.w >> 19) & 0x7fffffff) ^ (t ^ ((t >> 8) & 0x7fffffff))) & 0xffffffff;
        self.w as i32
    }

    pub fn next_int(&mut self) -> i32 {
        self.next_signed_int() & 0x7fffffff
    }

    pub fn next_float(&mut self) -> f64 {
        self.next_int() as f64 / 0x7fffffff as f64
    }

    pub fn next_signed_float(&mut self) -> f64 {
        self.next_signed_int() as f64 / 0x7fffffff as f64
    }

    pub fn next_boolean(&mut self) -> bool {
        (self.next_signed_int() & 0x01) == 0
    }

    pub fn next_range(&mut self, start: i32, end: i32) -> i32 {
        if end <= start {
            return start;
        }
        start + (self.next_int() % (end + 1 - start))
    }

    pub fn next_bounded_int(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            return 0;
        }
        self.next_int() % bound
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pocketmine_random_sequence() {
        let mut r = Random::new(12345);
        assert_eq!(r.next_int(), 499590199);
        assert_eq!(r.next_int(), 716643607);
        assert_eq!(r.next_int(), 611714095);
        assert_eq!(r.next_int(), 166706969);
        assert_eq!(r.next_int(), 757978255);
    }
}
