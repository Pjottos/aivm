#[cfg(feature = "cranelift")]
mod cranelift;
mod interpreter;
#[cfg(feature = "jit")]
mod jit;

#[cfg(feature = "cranelift")]
pub use self::cranelift::Cranelift;
pub use interpreter::Interpreter;
#[cfg(feature = "jit")]
pub use jit::Jit;

/// A converter to translate VM instructions to a form that can be executed on the host platform.
///
/// This trait is not meant to implemented outside this crate.
pub trait CodeGenerator: private::CodeGeneratorImpl {}

impl<T: private::CodeGeneratorImpl> CodeGenerator for T {}

pub(crate) mod private {
    use crate::{compile::CompareKind, Runner};

    use std::num::NonZeroU32;

    pub trait CodeGeneratorImpl {
        type Runner: Runner + 'static;
        type Emitter<'a>: Emitter + 'a
        where
            Self: 'a;

        fn begin(&mut self, function_count: NonZeroU32);
        fn begin_function(&mut self, idx: u32) -> Self::Emitter<'_>;
        fn finish(&mut self, memory_size: u32, output_size: u32, input_size: u32) -> Self::Runner;
    }

    pub trait Emitter {
        fn prepare_emit(&mut self) {}
        fn finalize(&mut self) {}

        fn emit_call(&mut self, idx: u32);
        fn emit_nop(&mut self);

        fn emit_int_add(&mut self, dst: u8, a: u8, b: u8);
        fn emit_int_sub(&mut self, dst: u8, a: u8, b: u8);
        fn emit_int_mul(&mut self, dst: u8, a: u8, b: u8);
        fn emit_int_mul_high(&mut self, dst: u8, a: u8, b: u8);
        fn emit_int_mul_high_unsigned(&mut self, dst: u8, a: u8, b: u8);
        fn emit_int_neg(&mut self, dst: u8, src: u8);
        fn emit_int_abs(&mut self, dst: u8, src: u8);
        fn emit_int_inc(&mut self, dst: u8);
        fn emit_int_dec(&mut self, dst: u8);
        fn emit_int_min(&mut self, dst: u8, a: u8, b: u8);
        fn emit_int_max(&mut self, dst: u8, a: u8, b: u8);

        fn emit_bit_or(&mut self, dst: u8, a: u8, b: u8);
        fn emit_bit_and(&mut self, dst: u8, a: u8, b: u8);
        fn emit_bit_xor(&mut self, dst: u8, a: u8, b: u8);
        fn emit_bit_not(&mut self, dst: u8, src: u8);
        fn emit_bit_shift_left(&mut self, dst: u8, src: u8, amount: u8);
        fn emit_bit_shift_right(&mut self, dst: u8, src: u8, amount: u8);
        fn emit_bit_rotate_left(&mut self, dst: u8, src: u8, amount: u8);
        fn emit_bit_rotate_right(&mut self, dst: u8, src: u8, amount: u8);
        fn emit_bit_select(&mut self, dst: u8, mask: u8, a: u8, b: u8);
        fn emit_bit_popcnt(&mut self, dst: u8, src: u8);
        fn emit_bit_reverse(&mut self, dst: u8, src: u8);

        fn emit_branch_cmp(&mut self, a: u8, b: u8, compare_kind: CompareKind, offset: u32);
        fn emit_branch_zero(&mut self, src: u8, offset: u32);
        fn emit_branch_non_zero(&mut self, src: u8, offset: u32);

        fn emit_mem_load(&mut self, dst: u8, addr: u32);
        fn emit_mem_store(&mut self, addr: u32, src: u8);
    }
}

#[cfg(test)]
mod tests {
    use super::{private::*, *};
    use crate::{compile::CompareKind, Runner};

    struct Harness<'a, G: CodeGeneratorImpl> {
        gen: G,
        next_func: u32,
        func_count: u32,
        mem: &'a mut [i64],
    }

    impl<'a, G: CodeGeneratorImpl> Harness<'a, G> {
        fn new(mut gen: G, func_count: u32, mem: &'a mut [i64]) -> Self {
            gen.begin(func_count.try_into().unwrap());
            Self {
                gen,
                next_func: 0,
                func_count,
                mem,
            }
        }

        fn run(mut self) {
            let runner = self.gen.finish(self.mem.len() as u32, 0, 0);
            runner.step(self.mem);
        }

        fn func<F: FnOnce(&mut G::Emitter<'_>)>(mut self, f: F) -> Self {
            assert!(self.next_func < self.func_count);
            {
                let mut e = self.gen.begin_function(self.next_func);
                f(&mut e);
                e.finalize();
            }
            self.next_func += 1;

            self
        }
    }

    macro_rules! instruction_tests {
        ($name:ident, $gen:expr) => {
            mod $name {
                use super::*;

                #[test]
                fn mem() {
                    let mut mem = [0x0DEADBEEDEADBEEF, 0];
                    Harness::new($gen, 1, &mut mem)
                        .func(|e| {
                            e.emit_mem_load(0, 0);
                            e.emit_mem_store(1, 0);
                        })
                        .run();

                    assert_eq!(mem[1], 0x0DEADBEEDEADBEEF);
                }

                #[test]
                fn int_mul_high() {
                    fn test_mul_high(a: i64, b: i64, result: i64) {
                        let mut mem = [a, b];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_mem_load(1, 1);
                                e.emit_int_mul_high(2, 0, 1);
                                e.emit_mem_store(0, 2);
                                e.emit_int_mul_high(2, 1, 0);
                                e.emit_mem_store(1, 2);
                            })
                            .run();

                        assert_eq!(mem[0], result);
                        assert_eq!(mem[1], result, "not commutative");
                    }

                    test_mul_high(-1, -1, 0);
                    test_mul_high(i64::MAX, -16, -8);
                    test_mul_high(-16, i64::MAX, -8);
                    test_mul_high(i64::MAX, 16, 7);
                    test_mul_high(16, i64::MAX, 7);
                    test_mul_high(i64::MIN, -16, 8);
                    test_mul_high(-16, i64::MIN, 8);
                    test_mul_high(i64::MIN, 16, -8);
                    test_mul_high(16, i64::MIN, -8);
                }

                #[test]
                fn int_mul_high_unsigned() {
                    fn test_mul_highu(a: i64, b: i64, result: i64) {
                        let mut mem = [a, b];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_mem_load(1, 1);
                                e.emit_int_mul_high_unsigned(2, 0, 1);
                                e.emit_mem_store(0, 2);
                                e.emit_int_mul_high_unsigned(2, 1, 0);
                                e.emit_mem_store(1, 2);
                            })
                            .run();

                        assert_eq!(mem[0], result);
                        assert_eq!(mem[1], result, "not commutative");
                    }

                    test_mul_highu(-1, -1, -2);
                    test_mul_highu(i64::MAX, -16, 0x7FFFFFFFFFFFFFF7);
                    test_mul_highu(-16, i64::MAX, 0x7FFFFFFFFFFFFFF7);
                    test_mul_highu(i64::MAX, 16, 7);
                    test_mul_highu(16, i64::MAX, 7);
                    test_mul_highu(i64::MIN, -16, 0x7FFFFFFFFFFFFFF8);
                    test_mul_highu(-16, i64::MIN, 0x7FFFFFFFFFFFFFF8);
                    test_mul_highu(i64::MIN, 16, 8);
                    test_mul_highu(16, i64::MIN, 8);
                }

                #[test]
                fn call() {
                    let mut mem = [0x0DEADBEEDEADBEEF, 0];
                    Harness::new($gen, 2, &mut mem)
                        .func(|e| {
                            e.emit_call(1);
                        })
                        .func(|e| {
                            e.emit_mem_load(0, 0);
                            e.emit_mem_store(1, 0);
                        })
                        .run();

                    assert_eq!(mem[1], 0x0DEADBEEDEADBEEF);
                }

                #[test]
                fn int_add() {
                    fn test_add(a: i64, b: i64) {
                        let mut mem = [a, b];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_mem_load(1, 1);
                                e.emit_int_add(2, 0, 1);
                                e.emit_mem_store(0, 2);
                                e.emit_int_add(2, 1, 0);
                                e.emit_mem_store(1, 2);
                            })
                            .run();

                        assert_eq!(mem[0], a.wrapping_add(b));
                        assert_eq!(mem[1], mem[0], "not commutative");
                    }

                    test_add(31, 11);
                    test_add(31, -11);
                    test_add(11, -31);
                    test_add(-31, -11);
                    test_add(i64::MIN, -1);
                    test_add(i64::MAX, 1);
                }

                #[test]
                fn int_sub() {
                    fn test_sub(a: i64, b: i64) {
                        let mut mem = [a, b];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_mem_load(1, 1);
                                e.emit_int_sub(2, 0, 1);
                                e.emit_mem_store(0, 2);
                                e.emit_int_sub(2, 1, 0);
                                e.emit_mem_store(1, 2);
                            })
                            .run();

                        assert_eq!(mem[0], a.wrapping_sub(b));
                        assert_eq!(mem[1], b.wrapping_sub(a));
                    }

                    test_sub(31, 11);
                    test_sub(31, -11);
                    test_sub(11, -31);
                    test_sub(-31, -11);
                    test_sub(i64::MIN, -1);
                    test_sub(i64::MAX, 1);
                }

                #[test]
                fn int_mul() {
                    fn test_mul(a: i64, b: i64) {
                        let mut mem = [a, b];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_mem_load(1, 1);
                                e.emit_int_mul(2, 0, 1);
                                e.emit_mem_store(0, 2);
                                e.emit_int_mul(2, 1, 0);
                                e.emit_mem_store(1, 2);
                            })
                            .run();

                        assert_eq!(mem[0], a.wrapping_mul(b));
                        assert_eq!(mem[1], mem[0], "not commutative");
                    }

                    test_mul(31, 11);
                    test_mul(31, -11);
                    test_mul(11, -31);
                    test_mul(-31, -11);
                    test_mul(i64::MAX, -1);
                    test_mul(-1, i64::MIN);
                }

                #[test]
                fn int_neg() {
                    fn test_neg(a: i64) {
                        let mut mem = [a];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_int_neg(0, 0);
                                e.emit_mem_store(0, 0);
                            })
                            .run();

                        assert_eq!(mem[0], a.wrapping_neg());
                    }

                    test_neg(1000);
                    test_neg(-1000);
                    test_neg(i64::MIN);
                }

                #[test]
                fn int_abs() {
                    fn test_abs(a: i64) {
                        let mut mem = [a];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_int_abs(0, 0);
                                e.emit_mem_store(0, 0);
                            })
                            .run();

                        assert_eq!(mem[0], a.wrapping_abs());
                    }

                    test_abs(1000);
                    test_abs(-1000);
                    test_abs(i64::MIN);
                }

                #[test]
                fn int_inc() {
                    fn test_inc(a: i64) {
                        let mut mem = [a];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_int_inc(0);
                                e.emit_mem_store(0, 0);
                            })
                            .run();

                        assert_eq!(mem[0], a.wrapping_add(1));
                    }

                    test_inc(1000);
                    test_inc(-1000);
                    test_inc(-1);
                    test_inc(i64::MAX);
                }

                #[test]
                fn int_dec() {
                    fn test_dec(a: i64) {
                        let mut mem = [a];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_int_dec(0);
                                e.emit_mem_store(0, 0);
                            })
                            .run();

                        assert_eq!(mem[0], a.wrapping_add(-1));
                    }

                    test_dec(1000);
                    test_dec(-1000);
                    test_dec(0);
                    test_dec(i64::MIN);
                }

                #[test]
                fn int_min() {
                    fn test_min(a: i64, b: i64) {
                        let mut mem = [a, b];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_mem_load(1, 1);
                                e.emit_int_min(2, 0, 1);
                                e.emit_mem_store(0, 2);
                                e.emit_int_min(2, 1, 0);
                                e.emit_mem_store(1, 2);
                            })
                            .run();

                        assert_eq!(mem[0], a.min(b));
                        assert_eq!(mem[1], mem[0], "not commutative");
                    }

                    test_min(31, 11);
                    test_min(31, -11);
                    test_min(11, -31);
                    test_min(-31, -11);
                    test_min(i64::MAX, -1);
                    test_min(-1, i64::MIN);
                }

                #[test]
                fn int_max() {
                    fn test_max(a: i64, b: i64) {
                        let mut mem = [a, b];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_mem_load(1, 1);
                                e.emit_int_max(2, 0, 1);
                                e.emit_mem_store(0, 2);
                                e.emit_int_max(2, 1, 0);
                                e.emit_mem_store(1, 2);
                            })
                            .run();

                        assert_eq!(mem[0], a.max(b));
                        assert_eq!(mem[1], mem[0], "not commutative");
                    }

                    test_max(31, 11);
                    test_max(31, -11);
                    test_max(11, -31);
                    test_max(-31, -11);
                    test_max(i64::MAX, -1);
                    test_max(-1, i64::MIN);
                }

                #[test]
                fn bit_or() {
                    fn test_or(a: i64, b: i64) {
                        let mut mem = [a, b];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_mem_load(1, 1);
                                e.emit_bit_or(2, 0, 1);
                                e.emit_mem_store(0, 2);
                                e.emit_bit_or(2, 1, 0);
                                e.emit_mem_store(1, 2);
                            })
                            .run();

                        assert_eq!(mem[0], a | b);
                        assert_eq!(mem[1], mem[0], "not commutative");
                    }

                    test_or(0x1F, 0x0B);
                    test_or(0, -1);
                    test_or(0, 0);
                    test_or(0x0F0F0F0F0F0F0F0F, 0xF0F0F0F0F0F0F0F0u64 as i64);
                }

                #[test]
                fn bit_and() {
                    fn test_and(a: i64, b: i64) {
                        let mut mem = [a, b];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_mem_load(1, 1);
                                e.emit_bit_and(2, 0, 1);
                                e.emit_mem_store(0, 2);
                                e.emit_bit_and(2, 1, 0);
                                e.emit_mem_store(1, 2);
                            })
                            .run();

                        assert_eq!(mem[0], a & b);
                        assert_eq!(mem[1], mem[0], "not commutative");
                    }

                    test_and(0x1F, 0x0F);
                    test_and(0, -1);
                    test_and(1, 2);
                    test_and(0x1F1F1F1F1F1F1F1F, 0xF2F2F2F2F2F2F2F2u64 as i64);
                }

                #[test]
                fn bit_xor() {
                    fn test_and(a: i64, b: i64) {
                        let mut mem = [a, b];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_mem_load(1, 1);
                                e.emit_bit_and(2, 0, 1);
                                e.emit_mem_store(0, 2);
                                e.emit_bit_and(2, 1, 0);
                                e.emit_mem_store(1, 2);
                            })
                            .run();

                        assert_eq!(mem[0], a & b);
                        assert_eq!(mem[1], mem[0], "not commutative");
                    }

                    test_and(0x1F, 0x0F);
                    test_and(0, -1);
                    test_and(1, 2);
                    test_and(0x1F1F1F1F1F1F1F1F, 0xF2F2F2F2F2F2F2F2u64 as i64);
                }

                #[test]
                fn bit_not() {
                    fn test_not(a: i64) {
                        let mut mem = [a];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_bit_not(0, 0);
                                e.emit_mem_store(0, 0);
                            })
                            .run();

                        assert_eq!(mem[0], !a);
                    }

                    test_not(1000);
                    test_not(-1000);
                    test_not(-1);
                    test_not(i64::MIN);
                    test_not(i64::MAX);
                }

                #[test]
                fn bit_shift_left() {
                    fn test_shift_left(a: i64, amount: u8) {
                        let mut mem = [a];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_bit_shift_left(0, 0, amount);
                                e.emit_mem_store(0, 0);
                            })
                            .run();

                        assert_eq!(mem[0], a << amount);
                    }

                    test_shift_left(8, 20);
                    test_shift_left(-1, 1);
                    test_shift_left(-1, 63);
                    test_shift_left(8, 0);
                    test_shift_left(i64::MIN, 1);
                    test_shift_left(i64::MAX, 15);
                }

                #[test]
                fn bit_shift_right() {
                    fn test_shift_right(a: i64, amount: u8) {
                        let mut mem = [a];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_bit_shift_right(0, 0, amount);
                                e.emit_mem_store(0, 0);
                            })
                            .run();

                        assert_eq!(mem[0], a >> amount);
                    }

                    test_shift_right(8, 20);
                    test_shift_right(-1, 1);
                    test_shift_right(-1, 63);
                    test_shift_right(-93, 3);
                    test_shift_right(i64::MIN, 63);
                    test_shift_right(i64::MAX, 63);
                }

                #[test]
                fn bit_rotate_left() {
                    fn test_rotate_left(a: i64, amount: u8) {
                        let mut mem = [a];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_bit_rotate_left(0, 0, amount);
                                e.emit_mem_store(0, 0);
                            })
                            .run();

                        assert_eq!(mem[0], a.rotate_left(amount as u32));
                    }

                    test_rotate_left(0x0101010101010101, 11);
                    test_rotate_left(0x0101010101010101, 59);
                    test_rotate_left(-93, 3);
                    test_rotate_left(i64::MIN, 63);
                    test_rotate_left(i64::MAX, 63);
                }

                #[test]
                fn bit_rotate_right() {
                    fn test_rotate_right(a: i64, amount: u8) {
                        let mut mem = [a];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_bit_rotate_right(0, 0, amount);
                                e.emit_mem_store(0, 0);
                            })
                            .run();

                        assert_eq!(mem[0], a.rotate_right(amount as u32));
                    }

                    test_rotate_right(0x0101010101010101, 11);
                    test_rotate_right(0x0101010101010101, 59);
                    test_rotate_right(-93, 3);
                    test_rotate_right(i64::MIN, 63);
                    test_rotate_right(i64::MAX, 63);
                }

                #[test]
                fn bit_select() {
                    fn test_select(mask: i64, a: i64, b: i64) {
                        let mut mem = [mask, a, b];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_mem_load(1, 1);
                                e.emit_mem_load(2, 2);
                                e.emit_bit_select(3, 0, 1, 2);
                                e.emit_mem_store(0, 3);
                            })
                            .run();

                        assert_eq!(mem[0], (mask & a) | (!mask & b));
                    }

                    test_select(0, 0x1F, 0x0F);
                    test_select(-1, 0x1F, 0x0F);
                    test_select(
                        0xAAAAAAAAAAAAAAAAu64 as i64,
                        0xDDDDDDDDDDDDDDDDu64 as i64,
                        0x6666666666666666,
                    );
                }

                #[test]
                fn bit_popcnt() {
                    fn test_popcnt(a: i64) {
                        let mut mem = [a];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_bit_popcnt(0, 0);
                                e.emit_mem_store(0, 0);
                            })
                            .run();

                        assert_eq!(mem[0], a.count_ones().into());
                    }

                    test_popcnt(0xF141010431510101u64 as i64);
                    test_popcnt(0x012345678ABCDEF1);
                    test_popcnt(-93);
                    test_popcnt(0);
                    test_popcnt(i64::MIN);
                    test_popcnt(i64::MAX);
                    test_popcnt(1);
                    test_popcnt(-1);
                }

                #[test]
                fn bit_reverse() {
                    fn test_reverse(a: i64) {
                        let mut mem = [a];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 0);
                                e.emit_bit_reverse(0, 0);
                                e.emit_mem_store(0, 0);
                            })
                            .run();

                        assert_eq!(mem[0], a.reverse_bits());
                    }

                    test_reverse(0xF141010431510101u64 as i64);
                    test_reverse(0x012345678ABCDEF1);
                    test_reverse(-93);
                    test_reverse(0);
                    test_reverse(i64::MIN);
                    test_reverse(i64::MAX);
                    test_reverse(1);
                    test_reverse(-1);
                }

                #[test]
                fn branch_cmp() {
                    fn test_branch_cmp(a: i64, b: i64, kind: CompareKind) {
                        let mut mem = [0, a, b, 0x0DEADBEEDEADBEEF];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 1);
                                e.emit_mem_load(1, 2);
                                e.emit_branch_cmp(0, 1, kind, 2);
                                e.emit_mem_load(3, 3);
                                e.emit_mem_store(0, 3);
                            })
                            .run();

                        let result = match kind {
                            CompareKind::Eq => a == b,
                            CompareKind::Neq => a != b,
                            CompareKind::Gt => a > b,
                            CompareKind::Lt => a < b,
                        };
                        let expected = if result { 0 } else { 0x0DEADBEEDEADBEEF };

                        assert_eq!(mem[0], expected);
                    }

                    test_branch_cmp(893, 893, CompareKind::Eq);
                    test_branch_cmp(892, 893, CompareKind::Eq);
                    test_branch_cmp(893, 892, CompareKind::Eq);
                    test_branch_cmp(893, 893, CompareKind::Neq);
                    test_branch_cmp(892, 893, CompareKind::Neq);
                    test_branch_cmp(893, 892, CompareKind::Neq);
                    test_branch_cmp(-1, 892, CompareKind::Gt);
                    test_branch_cmp(892, -1, CompareKind::Gt);
                    test_branch_cmp(0, -1, CompareKind::Gt);
                    test_branch_cmp(-1, -2, CompareKind::Gt);
                    test_branch_cmp(-2, -1, CompareKind::Gt);
                    test_branch_cmp(-1, 892, CompareKind::Lt);
                    test_branch_cmp(892, -1, CompareKind::Lt);
                    test_branch_cmp(0, -1, CompareKind::Lt);
                    test_branch_cmp(-1, -2, CompareKind::Lt);
                    test_branch_cmp(-2, -1, CompareKind::Lt);
                }

                #[test]
                fn branch_zero() {
                    fn test_branch_zero(a: i64) {
                        let mut mem = [0, a, 0x0DEADBEEDEADBEEF];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 1);
                                e.emit_branch_zero(0, 2);
                                e.emit_mem_load(2, 2);
                                e.emit_mem_store(0, 2);
                            })
                            .run();

                        let expected = if a == 0 { 0 } else { 0x0DEADBEEDEADBEEF };

                        assert_eq!(mem[0], expected);
                    }

                    test_branch_zero(0);
                    test_branch_zero(-1);
                    test_branch_zero(1);
                }

                #[test]
                fn branch_non_zero() {
                    fn test_branch_non_zero(a: i64) {
                        let mut mem = [0, a, 0x0DEADBEEDEADBEEF];
                        Harness::new($gen, 1, &mut mem)
                            .func(|e| {
                                e.emit_mem_load(0, 1);
                                e.emit_branch_non_zero(0, 2);
                                e.emit_mem_load(2, 2);
                                e.emit_mem_store(0, 2);
                            })
                            .run();

                        let expected = if a != 0 { 0 } else { 0x0DEADBEEDEADBEEF };

                        assert_eq!(mem[0], expected);
                    }

                    test_branch_non_zero(0);
                    test_branch_non_zero(-1);
                    test_branch_non_zero(1);
                }
            }
        };
    }

    instruction_tests!(interpreter_inst, Interpreter::new());
    #[cfg(feature = "cranelift")]
    instruction_tests!(cranelift_inst, Cranelift::new());
    #[cfg(feature = "jit")]
    instruction_tests!(jit_inst, Jit::new());
}
