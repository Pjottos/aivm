use rand::prelude::*;
use rand_pcg::{Pcg32, Pcg64};

mod mutate;

pub use mutate::fill_mutate_bits;

pub fn expand_code(root_seed: u64, mutation_seeds: &[u32], mutate_bits: &[u64], buf: &mut [u64]) {
    assert!(mutate_bits.len() >= buf.len());

    Pcg64::seed_from_u64(root_seed).fill(buf);

    let max_offset = u32::try_from(mutate_bits.len() - buf.len()).unwrap_or(u32::MAX);
    for seed in mutation_seeds.iter().copied() {
        let start = usize::try_from(seed % max_offset).unwrap();
        let end = start + buf.len();
        for (chunk, mutation) in buf.iter_mut().zip(&mutate_bits[start..end]) {
            *chunk ^= mutation;
        }
    }
}

pub fn expand_memory(root_seed: u64, mutation_seeds: &[u32], mutate_bits: &[u64], buf: &mut [i64]) {
    assert!(mutate_bits.len() >= buf.len());

    let mut rng = Pcg64::seed_from_u64(root_seed);
    Pcg64::seed_from_u64(rng.gen()).fill(buf);

    let max_offset = u32::try_from(mutate_bits.len() - buf.len()).unwrap_or(u32::MAX);
    for seed in mutation_seeds.iter().copied() {
        let seed = Pcg32::seed_from_u64(u64::from(seed)).gen::<u32>();
        let start = usize::try_from(seed % max_offset).unwrap();
        let end = start + buf.len();
        for (chunk, mutation) in buf.iter_mut().zip(mutate_bits[start..end].iter().copied()) {
            *chunk ^= mutation as i64;
        }
    }
}
