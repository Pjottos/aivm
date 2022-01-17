use rand::prelude::*;
use rand_pcg::Pcg64;

use std::slice;

mod mutate;

pub use mutate::mutate_code;

pub fn build_code_from_seeds(seeds: &[u64], p_mutate: u16, code_buf: &mut [u64]) {
    assert!(!seeds.is_empty());

    Pcg64::seed_from_u64(seeds[0]).fill(code_buf);

    for seed in seeds[1..].iter().copied() {
        mutate_code(code_buf, seed, p_mutate);
    }
}

pub fn build_memory_from_seeds(seeds: &[u64], p_mutate: u16, memory: &mut [i64]) {
    fn transform(seed: u64) -> u64 {
        Pcg64::seed_from_u64(seed).next_u64()
    }

    assert!(!seeds.is_empty());

    let memory =
        unsafe { slice::from_raw_parts_mut(memory.as_mut_ptr() as *mut u64, memory.len()) };

    Pcg64::seed_from_u64(transform(seeds[0])).fill(memory);

    for seed in seeds[1..].iter().copied() {
        mutate_code(memory, transform(seed), p_mutate);
    }
}
