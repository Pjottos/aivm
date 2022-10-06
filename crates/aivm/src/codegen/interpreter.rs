use crate::{codegen, compile::CompareKind};

use std::{
    convert::TryFrom,
    num::{NonZeroU32, Wrapping},
};

/// A code generator for creating a runner that simply interprets VM instructions one by one.
pub struct Interpreter {
    functions: Vec<Vec<Instruction>>,
}

impl codegen::private::CodeGeneratorImpl for Interpreter {
    type Runner = Runner;
    type Emitter<'a> = Emitter<'a>;

    fn begin(&mut self, function_count: NonZeroU32) {
        for func in &mut self.functions {
            func.clear();
        }

        self.functions
            .resize(usize::try_from(function_count.get()).unwrap(), vec![]);
    }

    fn begin_function(&mut self, idx: u32) -> Self::Emitter<'_> {
        Emitter {
            func: &mut self.functions[usize::try_from(idx).unwrap()],
        }
    }

    fn finish(&mut self, memory_size: u32, output_size: u32, input_size: u32) -> Self::Runner {
        let functions = self.functions.clone();

        Runner {
            functions,
            memory_size,
            output_size,
            input_size,
        }
    }
}

impl Interpreter {
    /// Create a new generator.
    pub fn new() -> Self {
        Self { functions: vec![] }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Runner {
    functions: Vec<Vec<Instruction>>,
    memory_size: u32,
    output_size: u32,
    input_size: u32,
}

impl crate::Runner for Runner {
    fn step(&self, memory: &mut [i64]) {
        assert!((self.memory_size + self.output_size + self.input_size) as usize <= memory.len());

        let output_range = memory.len() - self.output_size as usize..;
        memory[output_range].fill(0);

        self.call_function(memory, 0);
    }
}

impl Runner {
    fn call_function(&self, memory: &mut [i64], idx: u32) {
        use Instruction::*;

        let mut stack = [Wrapping(0i64); 64];
        let mut skip_count = 0;

        for instruction in self.functions[usize::try_from(idx).unwrap()]
            .iter()
            .copied()
        {
            if skip_count > 0 {
                skip_count -= 1;
                continue;
            }

            match instruction {
                Call { idx } => self.call_function(memory, idx),
                Nop => (),

                IntAdd { dst, a, b } => {
                    stack[usize::from(dst)] = stack[usize::from(a)] + stack[usize::from(b)]
                }
                IntSub { dst, a, b } => {
                    stack[usize::from(dst)] = stack[usize::from(a)] - stack[usize::from(b)]
                }
                IntMul { dst, a, b } => {
                    stack[usize::from(dst)] = stack[usize::from(a)] * stack[usize::from(b)]
                }
                IntMulHigh { dst, a, b } => {
                    let a = stack[usize::from(a)].0 as i128;
                    let b = stack[usize::from(b)].0 as i128;

                    stack[usize::from(dst)].0 = ((a * b) >> 64) as i64;
                }
                IntMulHighUnsigned { dst, a, b } => {
                    let a = stack[usize::from(a)].0 as u64 as u128;
                    let b = stack[usize::from(b)].0 as u64 as u128;

                    stack[usize::from(dst)].0 = ((a * b) >> 64) as i64;
                }
                IntNeg { dst, src } => stack[usize::from(dst)] = -stack[usize::from(src)],
                IntAbs { dst, src } => {
                    stack[usize::from(dst)].0 = stack[usize::from(src)].0.wrapping_abs()
                }
                IntInc { dst } => stack[usize::from(dst)] += Wrapping(1),
                IntDec { dst } => stack[usize::from(dst)] -= Wrapping(1),
                IntMin { dst, a, b } => {
                    stack[usize::from(dst)] = stack[usize::from(a)].min(stack[usize::from(b)])
                }
                IntMax { dst, a, b } => {
                    stack[usize::from(dst)] = stack[usize::from(a)].max(stack[usize::from(b)])
                }

                BitOr { dst, a, b } => {
                    stack[usize::from(dst)] = stack[usize::from(a)] | stack[usize::from(b)]
                }
                BitAnd { dst, a, b } => {
                    stack[usize::from(dst)] = stack[usize::from(a)] & stack[usize::from(b)]
                }
                BitXor { dst, a, b } => {
                    stack[usize::from(dst)] = stack[usize::from(a)] ^ stack[usize::from(b)]
                }
                BitNot { dst, src } => stack[usize::from(dst)] = !stack[usize::from(src)],
                BitShiftLeft { dst, src, amount } => {
                    stack[usize::from(dst)].0 = stack[usize::from(src)].0 << amount
                }
                BitShiftRight { dst, src, amount } => {
                    stack[usize::from(dst)].0 = stack[usize::from(src)].0 >> amount
                }
                BitRotateLeft { dst, src, amount } => {
                    stack[usize::from(dst)].0 =
                        stack[usize::from(src)].0.rotate_left(u32::from(amount))
                }
                BitRotateRight { dst, src, amount } => {
                    stack[usize::from(dst)].0 =
                        stack[usize::from(src)].0.rotate_right(u32::from(amount))
                }
                BitSelect { dst, mask, a, b } => {
                    let mask = stack[usize::from(mask)];
                    let a = stack[usize::from(a)];
                    let b = stack[usize::from(b)];

                    stack[usize::from(dst)] = (a & mask) | (b & !mask);
                }
                BitPopcnt { dst, src } => {
                    stack[usize::from(dst)].0 = i64::from(stack[usize::from(src)].0.count_ones())
                }
                BitReverse { dst, src } => {
                    stack[usize::from(dst)].0 = stack[usize::from(src)].0.reverse_bits()
                }

                BranchCmp {
                    a,
                    b,
                    compare_kind,
                    offset,
                } => {
                    let a = stack[usize::from(a)];
                    let b = stack[usize::from(b)];

                    let result = match compare_kind {
                        CompareKind::Eq => a == b,
                        CompareKind::Neq => a != b,
                        CompareKind::Gt => a > b,
                        CompareKind::Lt => a < b,
                    };

                    if result {
                        skip_count = offset;
                    }
                }
                BranchZero { src, offset } => {
                    if stack[usize::from(src)].0 == 0 {
                        skip_count = offset;
                    }
                }
                BranchNonZero { src, offset } => {
                    if stack[usize::from(src)].0 != 0 {
                        skip_count = offset;
                    }
                }

                MemLoad { dst, addr } => {
                    let idx = usize::try_from(addr).unwrap();
                    stack[usize::from(dst)].0 = memory[idx];
                }
                MemStore { addr, src } => {
                    let idx = usize::try_from(addr).unwrap();
                    memory[idx] = stack[usize::from(src)].0;
                }
            }
        }

        assert_eq!(skip_count, 0);
    }
}

#[derive(Debug, Clone, Copy)]
enum Instruction {
    Call {
        idx: u32,
    },
    Nop,

    IntAdd {
        dst: u8,
        a: u8,
        b: u8,
    },
    IntSub {
        dst: u8,
        a: u8,
        b: u8,
    },
    IntMul {
        dst: u8,
        a: u8,
        b: u8,
    },
    IntMulHigh {
        dst: u8,
        a: u8,
        b: u8,
    },
    IntMulHighUnsigned {
        dst: u8,
        a: u8,
        b: u8,
    },
    IntNeg {
        dst: u8,
        src: u8,
    },
    IntAbs {
        dst: u8,
        src: u8,
    },
    IntInc {
        dst: u8,
    },
    IntDec {
        dst: u8,
    },
    IntMin {
        dst: u8,
        a: u8,
        b: u8,
    },
    IntMax {
        dst: u8,
        a: u8,
        b: u8,
    },

    BitOr {
        dst: u8,
        a: u8,
        b: u8,
    },
    BitAnd {
        dst: u8,
        a: u8,
        b: u8,
    },
    BitXor {
        dst: u8,
        a: u8,
        b: u8,
    },
    BitNot {
        dst: u8,
        src: u8,
    },
    BitShiftLeft {
        dst: u8,
        src: u8,
        amount: u8,
    },
    BitShiftRight {
        dst: u8,
        src: u8,
        amount: u8,
    },
    BitRotateLeft {
        dst: u8,
        src: u8,
        amount: u8,
    },
    BitRotateRight {
        dst: u8,
        src: u8,
        amount: u8,
    },
    BitSelect {
        dst: u8,
        mask: u8,
        a: u8,
        b: u8,
    },
    BitPopcnt {
        dst: u8,
        src: u8,
    },
    BitReverse {
        dst: u8,
        src: u8,
    },

    BranchCmp {
        a: u8,
        b: u8,
        compare_kind: CompareKind,
        offset: u32,
    },
    BranchZero {
        src: u8,
        offset: u32,
    },
    BranchNonZero {
        src: u8,
        offset: u32,
    },

    MemLoad {
        dst: u8,
        addr: u32,
    },
    MemStore {
        addr: u32,
        src: u8,
    },
}

pub struct Emitter<'a> {
    func: &'a mut Vec<Instruction>,
}

impl<'a> codegen::private::Emitter for Emitter<'a> {
    fn emit_call(&mut self, idx: u32) {
        self.func.push(Instruction::Call { idx });
    }
    fn emit_nop(&mut self) {
        self.func.push(Instruction::Nop);
    }

    fn emit_int_add(&mut self, dst: u8, a: u8, b: u8) {
        self.func.push(Instruction::IntAdd { dst, a, b });
    }
    fn emit_int_sub(&mut self, dst: u8, a: u8, b: u8) {
        self.func.push(Instruction::IntSub { dst, a, b });
    }
    fn emit_int_mul(&mut self, dst: u8, a: u8, b: u8) {
        self.func.push(Instruction::IntMul { dst, a, b });
    }
    fn emit_int_mul_high(&mut self, dst: u8, a: u8, b: u8) {
        self.func.push(Instruction::IntMulHigh { dst, a, b });
    }
    fn emit_int_mul_high_unsigned(&mut self, dst: u8, a: u8, b: u8) {
        self.func
            .push(Instruction::IntMulHighUnsigned { dst, a, b });
    }
    fn emit_int_neg(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::IntNeg { dst, src });
    }
    fn emit_int_abs(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::IntAbs { dst, src });
    }
    fn emit_int_inc(&mut self, dst: u8) {
        self.func.push(Instruction::IntInc { dst });
    }
    fn emit_int_dec(&mut self, dst: u8) {
        self.func.push(Instruction::IntDec { dst });
    }
    fn emit_int_min(&mut self, dst: u8, a: u8, b: u8) {
        self.func.push(Instruction::IntMin { dst, a, b });
    }
    fn emit_int_max(&mut self, dst: u8, a: u8, b: u8) {
        self.func.push(Instruction::IntMax { dst, a, b });
    }

    fn emit_bit_or(&mut self, dst: u8, a: u8, b: u8) {
        self.func.push(Instruction::BitOr { dst, a, b });
    }
    fn emit_bit_and(&mut self, dst: u8, a: u8, b: u8) {
        self.func.push(Instruction::BitAnd { dst, a, b });
    }
    fn emit_bit_xor(&mut self, dst: u8, a: u8, b: u8) {
        self.func.push(Instruction::BitXor { dst, a, b });
    }
    fn emit_bit_not(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::BitNot { dst, src });
    }
    fn emit_bit_shift_left(&mut self, dst: u8, src: u8, amount: u8) {
        self.func
            .push(Instruction::BitShiftLeft { dst, src, amount });
    }
    fn emit_bit_shift_right(&mut self, dst: u8, src: u8, amount: u8) {
        self.func
            .push(Instruction::BitShiftRight { dst, src, amount });
    }
    fn emit_bit_rotate_left(&mut self, dst: u8, src: u8, amount: u8) {
        self.func
            .push(Instruction::BitRotateLeft { dst, src, amount });
    }
    fn emit_bit_rotate_right(&mut self, dst: u8, src: u8, amount: u8) {
        self.func
            .push(Instruction::BitRotateRight { dst, src, amount });
    }
    fn emit_bit_select(&mut self, dst: u8, mask: u8, a: u8, b: u8) {
        self.func.push(Instruction::BitSelect { dst, mask, a, b });
    }
    fn emit_bit_popcnt(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::BitPopcnt { dst, src });
    }
    fn emit_bit_reverse(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::BitReverse { dst, src });
    }

    fn emit_branch_cmp(&mut self, a: u8, b: u8, compare_kind: CompareKind, offset: u32) {
        self.func.push(Instruction::BranchCmp {
            a,
            b,
            compare_kind,
            offset,
        });
    }
    fn emit_branch_zero(&mut self, src: u8, offset: u32) {
        self.func.push(Instruction::BranchZero { src, offset });
    }
    fn emit_branch_non_zero(&mut self, src: u8, offset: u32) {
        self.func.push(Instruction::BranchNonZero { src, offset });
    }

    fn emit_mem_load(&mut self, dst: u8, addr: u32) {
        self.func.push(Instruction::MemLoad { dst, addr });
    }
    fn emit_mem_store(&mut self, addr: u32, src: u8) {
        self.func.push(Instruction::MemStore { addr, src });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        codegen::private::{CodeGeneratorImpl, Emitter},
        Runner,
    };

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

    #[test]
    fn mem() {
        let mut mem = [0x0DEADBEEDEADBEEF, 0];
        Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
        Harness::new(Interpreter::new(), 2, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
            Harness::new(Interpreter::new(), 1, &mut mem)
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
