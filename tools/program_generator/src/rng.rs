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
pub struct Rng {
    rng: Mutex<Xoroshiro64Star>,
}

impl Rng {
    /// Create a new Rand object
    pub fn new() -> Self {
        Self {
            rng: Mutex::new(Xoroshiro64Star::seed_from_u64(rdtsc()))
        }
    }

    /// Return a random number
    pub fn gen(&self) -> usize {
        self.rng.lock().unwrap().next_u64() as usize
    }

    /// Return a random byte-string, ascii-range 1-0xff (inclusive)
    pub fn next_string(&self, max_length: usize, min: usize, max: usize) -> Vec<u8> {
        // Create a random byte-string
        let rand_bytes = (1..max_length).map(|_| {
                self.gen_range(min, max) as u8
            }).collect::<Vec<u8>>();
        
        rand_bytes
    }

    /// Return 2 random 32-bit unsigned integers
    pub fn get2_rand(&self) -> (usize, usize) {
        let tmp = self.rng.lock().unwrap().next_u64();
        ((tmp & 0xffffffff) as usize, (tmp >> 32) as usize)
    }

    /// Return a random number up to `max`
    pub fn next_num(&self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        self.rng.lock().unwrap().next_u64() as usize % max
    }

    /// Return a random value in the range min..max, inclusive of min and exclusive of max
    pub fn gen_range(&self, min: usize, max: usize) -> usize {
        if max == min {
            return min;
        }

        (self.rng.lock().unwrap().next_u64() as usize % (max - min)) + min
    }
}
