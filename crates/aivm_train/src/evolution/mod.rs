use rand::prelude::*;
use rand_pcg::Pcg64;

mod mutate;

pub use mutate::mutate_code;

pub fn build_code_from_seeds(seeds: &[u64], p_mutate: u16, code_buf: &mut [u64]) {
    assert!(!seeds.is_empty());

    Pcg64::seed_from_u64(seeds[0]).fill(code_buf);

    for seed in seeds[1..].iter().copied() {
        mutate_code(code_buf, seed, p_mutate);
    }
}
