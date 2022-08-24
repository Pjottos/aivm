use crate::{
    codegen::{self, private::MemoryBank},
    compile::CompareKind,
};

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

    fn finish(&mut self, input_size: u32, output_size: u32, memory_size: u32) -> Self::Runner {
        let functions = self.functions.clone();

        Runner {
            functions,
            input_size,
            output_size,
            memory_size,
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
    input_size: u32,
    output_size: u32,
    memory_size: u32,
}

impl crate::Runner for Runner {
    fn step(&self, input: &[i64], output: &mut [i64], memory: &mut [i64]) {
        assert!(self.input_size as usize <= input.len());
        assert!(self.output_size as usize <= output.len());
        assert!(self.memory_size as usize <= memory.len());
        self.call_function(input, output, memory, 0);
    }
}

impl Runner {
    fn call_function(&self, input: &[i64], output: &mut [i64], memory: &mut [i64], idx: u32) {
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
                Call { idx } => self.call_function(input, output, memory, idx),
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
                    let a = stack[usize::from(a)].0 as u128;
                    let b = stack[usize::from(b)].0 as u128;

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
                    stack[usize::from(dst)] = stack[usize::from(a)].min(stack[usize::from(b)])
                }

                BitSwap { dst, src } => stack.swap(usize::from(dst), usize::from(src)),
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
                    stack[usize::from(dst)].0 =
                        ((stack[usize::from(src)].0 as u64) >> amount) as i64
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

                MemLoad { bank, dst, addr } => {
                    let idx = usize::try_from(addr).unwrap();
                    let target = &mut stack[usize::from(dst)].0;
                    match bank {
                        MemoryBank::Input => *target = input[idx],
                        MemoryBank::Memory => *target = memory[idx],
                        MemoryBank::Output => panic!("tried to load from output"),
                    }
                }
                MemStore { bank, addr, src } => {
                    let idx = usize::try_from(addr).unwrap();
                    let val = stack[usize::from(src)].0;
                    match bank {
                        MemoryBank::Output => output[idx] = val,
                        MemoryBank::Memory => memory[idx] = val,
                        MemoryBank::Input => panic!("tried to store to input"),
                    }
                }
            }
        }
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

    BitSwap {
        dst: u8,
        src: u8,
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
        bank: MemoryBank,
        dst: u8,
        addr: u32,
    },
    MemStore {
        bank: MemoryBank,
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

    fn emit_bit_swap(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::BitSwap { dst, src });
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

    fn emit_mem_load(&mut self, bank: MemoryBank, dst: u8, addr: u32) {
        self.func.push(Instruction::MemLoad { bank, dst, addr });
    }
    fn emit_mem_store(&mut self, bank: MemoryBank, addr: u32, src: u8) {
        self.func.push(Instruction::MemStore { bank, addr, src });
    }
}
