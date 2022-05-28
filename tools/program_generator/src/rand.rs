//! This is used to expose an api for an rng-object that can safely be used in a global variable.
//! This required locks, which makes it quite slow. 

use std::sync::Mutex;

use rand_xoshiro::rand_core::RngCore;
use rand_xoshiro::Xoroshiro64Star;
use rand_xoshiro::rand_core::SeedableRng;

/// Used to seed randomness based on cpu timestamp
fn rdtsc() -> u64 {
    unsafe { std::arch::x86_64::_rdtsc() }
}

/// Randomness exposing api that can be used in a global and uses a faster rand implementation than
/// the standard Rand crate
pub struct Rand {
    rng: Mutex<Xoroshiro64Star>,
}

impl Rand {
    /// Create a new Rand object
    pub fn new() -> Self {
        Self {
            rng: Mutex::new(Xoroshiro64Star::seed_from_u64(rdtsc()))
        }
    }

    /// Return 2 random 32-bit unsigned integers
    pub fn get2_rand(&self) -> (usize, usize) {
        let tmp = self.rng.lock().unwrap().next_u64();
        ((tmp & 0xffffffff) as usize, (tmp >> 32) as usize)
    }

    /// Return a random number up to `max`
    pub fn next_num(&self, max: usize) -> usize {
        self.rng.lock().unwrap().next_u64() as usize % max
    }

    /// Return a random value in the range min..max, inclusive of min and exclusive of max
    pub fn gen_range(&self, min: usize, max: usize) -> usize {
        (self.rng.lock().unwrap().next_u64() as usize % (max - min)) + min
    }
}
