use rand::prelude::*;
use rand_pcg::Pcg64;

pub fn mutate_code(code: &mut [u64], seed: u64, p_mutate: u16) {
    let mut rng = Pcg64::seed_from_u64(seed);

    for instruction in code {
        let mut mutations = 0;

        for _ in 0..16 {
            let rand = rng.next_u64();

            // TODO: use simd when it's stable
            mutations <<= 1;
            for i in 0..4 {
                let shift_amount = i * 16;
                let mutate_bit = ((rand >> shift_amount) as u16) < p_mutate;
                mutations |= (mutate_bit as u64) << shift_amount;
            }
        }

        *instruction ^= mutations;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_determinism() {
        let mut code = [0; 32];
        let seed = 33;
        let p_mutate = 1024;

        mutate_code(&mut code, seed, p_mutate);

        assert_eq!(
            code,
            [
                18014398509481984,
                0,
                70368744177664,
                2048,
                17039360,
                0,
                0,
                4398046511104,
                4611686018427387912,
                1152921504606846976,
                4096,
                2097408,
                16384,
                33554432,
                128,
                128,
                4503600701112320,
                0,
                0,
                0,
                36028797018964000,
                134217728,
                9223407289946341376,
                281474976710672,
                281474976710656,
                0,
                1154047438873427968,
                320,
                0,
                0,
                37383395352576,
                0,
            ],
        );
    }
}
