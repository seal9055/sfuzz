//use crate::{
//};

use rand::Rng;
use rand::rngs::ThreadRng;

#[derive(Debug, Clone)]
pub struct Mutator {
    rng: ThreadRng,
}

impl Mutator {
    pub fn new(rng: ThreadRng) -> Self {
        Self {
            rng,

        }
    }

    pub fn mutate(&mut self, input: &mut [u8]) {
        let input_length = input.len();

        for _ in 0..self.rng.gen_range(0..8) {
            input[self.rng.gen_range(0..input_length)] = self.rng.gen_range(0..255) as u8;
        }
    }
}
