use rand_xoshiro::rand_core::RngCore;
use rand_xoshiro::Xoroshiro64Star;
use rand_xoshiro::rand_core::SeedableRng;

const MUTATE_SIMPLE: bool = false;

#[derive(Copy, Clone, Debug)]
pub enum Mutation {
    ByteReplace,
    BitFlip,
    MagicNum,
    SimpleArithmetic,
    RemoveBlock,
    DupBlock,
    Resize,
}

#[derive(Debug, Clone)]
pub struct Mutator {
    /// Fast Rng
    rng: Xoroshiro64Star,

    /// Available mutation strategies
    mutation_strats: Vec<Mutation>,

    /// Count-down to havoc mode
    havoc_counter: usize,
}

impl Mutator {
    pub fn default() -> Self {
        // Initialize the individual strategies for the mutation_strats array alongside their
        // weight. This creates a larger array since weight is created by inserting new
        // elements into the array, but I believe that this should be much faster than
        // alternatives
        let mut mut_strats: Vec<Mutation> = Vec::new();
        mut_strats.append(&mut (0..1000).map(|_| { Mutation::ByteReplace }).collect());
        mut_strats.append(&mut (0..1000).map(|_| { Mutation::BitFlip }).collect());
        mut_strats.append(&mut (0..200).map(|_|  { Mutation::MagicNum }).collect());
        mut_strats.append(&mut (0..500).map(|_|  { Mutation::SimpleArithmetic }).collect());
        mut_strats.append(&mut (0..30).map(|_|   { Mutation::RemoveBlock }).collect());
        mut_strats.append(&mut (0..30).map(|_|   { Mutation::DupBlock }).collect());
        mut_strats.append(&mut (0..10).map(|_|   { Mutation::Resize }).collect());

        Self {
            rng: Xoroshiro64Star::seed_from_u64(0),
            mutation_strats: mut_strats,
            havoc_counter: 0,
        }
    }

    /// Return 2 random 32-bit unsigned integers
    fn get2_rand(&mut self) -> (usize, usize) {
        let tmp = self.rng.next_u64();
        ((tmp & 0xffffffff) as usize, (tmp >> 32) as usize)
    }

    /// Chose a set of random bytes and mutate them. Prefer small corruption over larger one's
    fn byte_replace(&mut self, input: &mut [u8]) -> Result<(),()> {
        let input_length = input.len();
        let (r1, r2) = self.get2_rand();

        if (r1 % 1000) < 950 {
            // Small corruption, 0-32 bytes
            for _ in 1..(r2 % 32) {
                let (r1, r2) = self.get2_rand();
                input[(r1 % input_length)] = r2 as u8;
            }
        } else {
            // Larger corruption, 64-128 bytes
            for _ in 64..(64 + (r2 % 64)) {
                let (r1, r2) = self.get2_rand();
                input[(r1 % input_length)] = r2 as u8;
            }
        }
        Ok(())
    }

    /// Flip some random bits in the input
    fn bit_flip(&mut self, input: &mut [u8]) -> Result<(),()> {
        let input_length = input.len();
        let (r1, r2) = self.get2_rand();

        if (r1 % 1000) < 950 {
            // Small corruption, flip up to 32 bits
            for _ in 1..(r2 % 32) {
                let (r1, r2) = self.get2_rand();
                let bit_idx = r1 % 8;
                input[(r2 % input_length)] ^= 1 << bit_idx;
            }
        } else {
            // Larger corruption, flip 64-128 bits
            for _ in 64..(64 + (r2 % 64)) {
                let (r1, r2) = self.get2_rand();
                let bit_idx = r1 % 8;
                input[(r2 % input_length)] ^= 1 << bit_idx;
            }
        }
        Ok(())
    }

    /// Replace 1/2/4/8 bytes in the program with values that are likely to cause bugs (eg. 0 or
    /// INT_MAX)
    fn magic_nums(&mut self, input: &mut Vec<u8>) -> Result<(),()> {
        // Just return if input is too small to operate on in a useful manner
        if input.len() < 32 { return Err(()); }

        let (r1, r2) = self.get2_rand();
        let splice_start = r1 % (input.len() - 8);
        let magic_nums: Vec<Vec<u8>> = vec![
            vec![0x0], vec![0x0; 2], vec![0x0; 4], vec![0x0; 8],
            vec![0xff], vec![0xff; 2], vec![0xff; 4], vec![0xff; 8],
            vec![0x7f], vec![0x7f, 0xff], vec![0x7f, 0xff, 0xff, 0xff],
            vec![0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff], vec![0x01, 0x0, 0x0],
        ];

        input.splice(splice_start.., magic_nums[r2 % magic_nums.len()].iter().cloned());
        Ok(())
    }

    /// Add or subtract some bytes to attempt to cause an integer over/underflow
    fn simple_arithmetic(&mut self, input: &mut [u8]) -> Result<(),()> {
        let input_length = input.len();

        let (r1, r2) = self.get2_rand();

        if (r1 % 1000) < 950 {
            // Small corruption, 0-32 bytes, 50% chance to either add or sub a value 0-32
            for i in 1..(r2 % 32) {
                let (r1, r2) = self.get2_rand();
                if i & 1 == 0 {
                    input[(r1 % input_length)] = 
                        input[(r1 % input_length)].wrapping_add((r2 % 32) as u8);
                } else {
                    input[(r1 % input_length)] = 
                        input[(r1 % input_length)].wrapping_sub((r2 % 32) as u8);
                }
            }
        } else {
            // Larger corruption, 64-128 bytes, 50% chance to either add or sub a value 0-32
            for i in 64..(64 + (r2 % 64)) {
                let (r1, r2) = self.get2_rand();
                if i & 1 == 0 {
                    input[(r1 % input_length)] = 
                        input[(r1 % input_length)].wrapping_add((r2 % 32) as u8);
                } else {
                    input[(r1 % input_length)] = 
                        input[(r1 % input_length)].wrapping_sub((r2 % 32) as u8);
                }
            }
        }
        Ok(())
    }

    /// Remove part of the input
    fn remove_block(&mut self, input: &mut Vec<u8>) -> Result<(),()> {
        let input_length = input.len();

        // Just return if input is already extremely small
        if input_length < 32 { return Err(()); }

        let (r1, r2) = self.get2_rand();

        let start = r1 % input_length;
        let end   = start + core::cmp::min(input_length - start, r2 % 512);

        // Refuse to perform this mutation if input would end up too small
        if (input_length - (end - start)) < 32 { return Err(()); }

        input.drain(start..end);
        Ok(())
    }

    /// Take a random block out of the input and duplicate it into a different location of the
    /// input
    fn duplicate_block(&mut self, input: &mut Vec<u8>) -> Result<(),()> {
        let input_length = input.len();

        // Just return if input is too small to operate on in a useful manner
        if input_length < 32 { return Err(()); }

        let (r1, r2) = self.get2_rand();

        // Calculate a random range within the input
        let start = r1 % input_length;
        let end   = start + core::cmp::min(input_length - start, r2 % 128);

        // Chose random location to insert dup'd block into
        let idx = self.rng.next_u32() as usize % input_length;

        // Extract block to be dup'd and split the input at a random location
        let mut v = input[start..end].to_vec();
        let mut p2 = input.split_off(idx);

        // Rebuild the input [(0..idx) + v + (idx..end)]
        input.append(&mut v);
        input.append(&mut p2);
        Ok(())
    }

    /// Resize the input, can both truncate, or add random bytes into the middle of an input
    fn resize(&mut self, input: &mut Vec<u8>) -> Result<(),()> {
        let input_length = input.len();
        let (r1, r2) = self.get2_rand();

        if r1 & 1 == 0 { // Truncate
            // Just return if input is too small to operate on in a useful manner
            if input_length < 32 { return Err(()); }
            let trunc_val = (r2 % (input_length / 2)) % 512;

            // Refuse to perform this mutation if input would end up too small
            if trunc_val < 32 { return Err(()); }
            
            input.truncate(trunc_val);
        } else { // Increase size
            let size = if input_length < 32 {
                32
            } else {
                (r2 % (input_length / 2)) % 512
            };

            // Create a set of random bytes that we can append to the input
            let rand_bytes = (0..(size / 8)).map(|_| {
                self.rng.next_u64()
            }).collect::<Vec<u64>>();

            // Transform these bytes from Vec<u64> to Vec<u8>
            let mut as_u8: Vec<u8> = unsafe {
                std::slice::from_raw_parts(
                    rand_bytes.as_ptr() as *const u8,
                    rand_bytes.len() * std::mem::size_of::<u64>(),
                ).to_vec()
            };
            input.append(&mut as_u8);
        }
        Ok(())
    }

    /// Chose a random mutation strategy
    fn chose_mut(&mut self) -> Mutation {
        let tmp_rand = self.rng.next_u32() as usize % self.mutation_strats.len();
        self.mutation_strats[tmp_rand]
    }

    /// Apply various implemented mutation strategies. Every 100 cases, use 'havoc-mode' which
    /// applies multiple strategies at the same time
    fn mutate_complex(&mut self, input: &mut Vec<u8>) {
        let mut muts = Vec::new();
        self.havoc_counter += 1;

        // Usually only perform 1 mutation, but if havoc is invoked, we queue up multiple
        // mutations onto the input in this fuzz-case
        if self.havoc_counter == 100 {
            self.havoc_counter = 0;
            for _ in 1..(self.rng.next_u32() % 8) {
                muts.push(self.chose_mut());
            }
        } else {
            muts.push(self.chose_mut());
        }

        for mutation in &mut muts {
            'inner: loop {
                let res = match mutation {
                    Mutation::ByteReplace      => self.byte_replace(input),
                    Mutation::BitFlip          => self.bit_flip(input),
                    Mutation::MagicNum        => self.magic_nums(input),
                    Mutation::SimpleArithmetic => self.simple_arithmetic(input),
                    Mutation::RemoveBlock      => self.remove_block(input),
                    Mutation::DupBlock         => self.duplicate_block(input),
                    Mutation::Resize           => self.resize(input),
                };

                // If the chosen strategy failed, chose a different mutation and rerun the
                // mutator, otherwise break out of the inner loop to keep the mutation result
                if res.is_ok() { 
                    break 'inner; 
                } else {
                    *mutation = self.chose_mut();
                }
            }
        }
    }

    /// Perform extremely simple/fast mutations
    fn mutate_simple(&mut self, input: &mut [u8]) {
        let input_length = input.len();

        for _ in 0..(self.rng.next_u32() % 8) {
            let (r1, r2) = self.get2_rand();
            input[(r1 % input_length)] = r2 as u8;
        }
    }

    /// Start the mutation process of an input
    pub fn mutate(&mut self, input: &mut Vec<u8>) {
        if MUTATE_SIMPLE {
            self.mutate_simple(input);
        } else {
            self.mutate_complex(input);
        }
    }
}

