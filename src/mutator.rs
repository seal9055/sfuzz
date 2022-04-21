//use crate::{
//};

use rand_xoshiro::Xoroshiro64Star;
use rand_xoshiro::rand_core::RngCore;

#[derive(Debug, Clone)]
pub struct Mutator {
    rng: Xoroshiro64Star,
}

impl Mutator {
    pub fn new(rng: Xoroshiro64Star) -> Self {
        Self {
            rng,

        }
    }

    pub fn mutate(&mut self, input: &mut [u8]) {
        let input_length = input.len();

        for _ in 0..(self.rng.next_u32() % 8) {
            input[(self.rng.next_u32() as usize % input_length)] 
                = (self.rng.next_u32() % 255) as u8;
        }
    }
}
