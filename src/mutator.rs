use crate::{
};

use rand::Rng;

#[derive(Copy, Debug, Clone, Eq, PartialEq)]
pub struct Mutator {

}

impl Mutator {
    pub fn new() -> Self {
        Self {

        }
    }

    pub fn mutate(self, input: &mut Vec<u8>) {
        let mut rng = rand::thread_rng();
        let input_length = input.len();

        for _ in 0..rng.gen_range(0..8) {
            input[rng.gen_range(0..input_length)] = rng.gen_range(0..255) as u8;
        }
    }
}
