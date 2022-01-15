use crate::{
    codegen,
    compile::{BranchParams, CompareKind},
};

use std::num::{NonZeroUsize, Wrapping};

pub struct Interpreter {
    functions: Vec<Vec<Instruction>>,
}

impl codegen::private::CodeGeneratorImpl for Interpreter {
    type Runner = Runner;
    type Emitter<'a> = Emitter<'a>;

    fn begin(&mut self, function_count: NonZeroUsize) {
        for func in &mut self.functions {
            func.clear();
        }

        self.functions.resize(function_count.get(), vec![]);
    }

    fn begin_function(&mut self, idx: usize) -> Self::Emitter<'_> {
        Emitter {
            func: &mut self.functions[idx],
        }
    }

    fn finish(&mut self, memory: Vec<i64>) -> Self::Runner {
        let functions = self.functions.clone();

        Runner { functions, memory }
    }
}

impl Interpreter {
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
    memory: Vec<i64>,
}

impl crate::Runner for Runner {
    fn step(&mut self) {
        Self::call_function(&self.functions, &mut self.memory, 0);
    }

    fn memory(&self) -> &[i64] {
        &self.memory
    }

    fn memory_mut(&mut self) -> &mut [i64] {
        &mut self.memory
    }
}

impl Runner {
    fn call_function(functions: &[Vec<Instruction>], memory: &mut [i64], idx: usize) {
        use Instruction::*;

        let mut stack = [Wrapping(0i64); 256];
        let mut skip_count = 0;

        for instruction in functions[idx].iter().copied() {
            while skip_count > 0 {
                skip_count -= 1;
                continue;
            }

            match instruction {
                Call(idx) => Self::call_function(functions, memory, idx),
                Nop => (),

                IntAdd { dst, src } => stack[dst as usize] += stack[src as usize],
                IntSub { dst, src } => stack[dst as usize] -= stack[src as usize],
                IntMul { dst, src } => stack[dst as usize] *= stack[src as usize],
                IntMulHigh { dst, src } => {
                    let d = stack[dst as usize].0 as i128;
                    let s = stack[src as usize].0 as i128;

                    stack[dst as usize] = Wrapping(((d * s) >> 64) as i64);
                }
                IntMulHighUnsigned { dst, src } => {
                    let d = stack[dst as usize].0 as u128;
                    let s = stack[src as usize].0 as u128;

                    stack[dst as usize] = Wrapping(((d * s) >> 64) as i64);
                }
                IntNeg { dst } => stack[dst as usize] = -stack[dst as usize],

                BitSwap { dst, src } => stack.swap(dst as usize, src as usize),
                BitOr { dst, src } => stack[dst as usize] |= stack[src as usize],
                BitAnd { dst, src } => stack[dst as usize] &= stack[src as usize],
                BitXor { dst, src } => stack[dst as usize] ^= stack[src as usize],
                BitShiftLeft { dst, amount } => stack[dst as usize].0 <<= amount,
                BitShiftRight { dst, amount } => stack[dst as usize].0 >>= amount,
                BitRotateLeft { dst, amount } => {
                    stack[dst as usize].0 = stack[dst as usize].0.rotate_left(amount as u32)
                }
                BitRotateRight { dst, amount } => {
                    stack[dst as usize].0 = stack[dst as usize].0.rotate_right(amount as u32)
                }

                CondBranch { a, b, params } => {
                    let a = stack[a as usize];
                    let b = stack[b as usize];

                    let result = match params.compare_kind() {
                        CompareKind::Eq => a == b,
                        CompareKind::Neq => a != b,
                        CompareKind::Gt => a > b,
                        CompareKind::Lt => a < b,
                    };

                    if result {
                        skip_count = params.offset();
                    }
                }

                MemLoad { dst, addr } => stack[dst as usize].0 = memory[addr],
                MemStore { addr, src } => memory[addr] = stack[src as usize].0,
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Instruction {
    Call(usize),
    Nop,

    IntAdd { dst: u8, src: u8 },
    IntSub { dst: u8, src: u8 },
    IntMul { dst: u8, src: u8 },
    IntMulHigh { dst: u8, src: u8 },
    IntMulHighUnsigned { dst: u8, src: u8 },
    IntNeg { dst: u8 },

    BitSwap { dst: u8, src: u8 },
    BitOr { dst: u8, src: u8 },
    BitAnd { dst: u8, src: u8 },
    BitXor { dst: u8, src: u8 },
    BitShiftLeft { dst: u8, amount: u8 },
    BitShiftRight { dst: u8, amount: u8 },
    BitRotateLeft { dst: u8, amount: u8 },
    BitRotateRight { dst: u8, amount: u8 },

    CondBranch { a: u8, b: u8, params: BranchParams },

    MemLoad { dst: u8, addr: usize },
    MemStore { addr: usize, src: u8 },
}

pub struct Emitter<'a> {
    func: &'a mut Vec<Instruction>,
}

impl<'a> codegen::private::Emitter for Emitter<'a> {
    fn emit_call(&mut self, idx: usize) {
        self.func.push(Instruction::Call(idx));
    }

    fn emit_nop(&mut self) {
        self.func.push(Instruction::Nop);
    }

    fn emit_int_add(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::IntAdd { dst, src });
    }

    fn emit_int_sub(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::IntSub { dst, src });
    }

    fn emit_int_mul(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::IntMul { dst, src });
    }

    fn emit_int_mul_high(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::IntMulHigh { dst, src });
    }

    fn emit_int_mul_high_unsigned(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::IntMulHighUnsigned { dst, src });
    }

    fn emit_int_neg(&mut self, dst: u8) {
        self.func.push(Instruction::IntNeg { dst });
    }

    fn emit_bit_swap(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::BitSwap { dst, src });
    }

    fn emit_bit_or(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::BitOr { dst, src });
    }

    fn emit_bit_and(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::BitAnd { dst, src });
    }

    fn emit_bit_xor(&mut self, dst: u8, src: u8) {
        self.func.push(Instruction::BitXor { dst, src });
    }

    fn emit_bit_shift_left(&mut self, dst: u8, amount: u8) {
        self.func.push(Instruction::BitShiftLeft { dst, amount });
    }

    fn emit_bit_shift_right(&mut self, dst: u8, amount: u8) {
        self.func.push(Instruction::BitShiftRight { dst, amount });
    }

    fn emit_bit_rotate_left(&mut self, dst: u8, amount: u8) {
        self.func.push(Instruction::BitRotateLeft { dst, amount });
    }

    fn emit_bit_rotate_right(&mut self, dst: u8, amount: u8) {
        self.func.push(Instruction::BitRotateRight { dst, amount });
    }

    fn emit_cond_branch(&mut self, a: u8, b: u8, params: BranchParams) {
        self.func.push(Instruction::CondBranch { a, b, params });
    }

    fn emit_mem_load(&mut self, dst: u8, addr: usize) {
        self.func.push(Instruction::MemLoad { dst, addr });
    }

    fn emit_mem_store(&mut self, addr: usize, src: u8) {
        self.func.push(Instruction::MemStore { addr, src });
    }
}
