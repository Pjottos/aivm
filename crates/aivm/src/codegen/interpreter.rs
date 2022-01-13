use crate::{
    codegen::CodeGeneratorImpl,
    compile::{BranchParams, CompareKind},
    Runner,
};

use core::num::Wrapping;

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

pub struct Interpreter {
    functions: Vec<Vec<Instruction>>,
    memory: Option<Vec<i64>>,
    cur_function: usize,
}

impl CodeGeneratorImpl for Interpreter {
    type Runner = Self;

    fn create(function_count: usize) -> Self {
        Self {
            functions: vec![vec![]; function_count],
            memory: None,
            cur_function: 0,
        }
    }

    fn emit_call(&mut self, idx: usize) {
        self.functions[self.cur_function].push(Instruction::Call(idx));
    }

    fn emit_nop(&mut self) {
        self.functions[self.cur_function].push(Instruction::Nop);
    }

    fn emit_int_add(&mut self, dst: u8, src: u8) {
        self.functions[self.cur_function].push(Instruction::IntAdd { dst, src });
    }

    fn emit_int_sub(&mut self, dst: u8, src: u8) {
        self.functions[self.cur_function].push(Instruction::IntSub { dst, src });
    }

    fn emit_int_mul(&mut self, dst: u8, src: u8) {
        self.functions[self.cur_function].push(Instruction::IntMul { dst, src });
    }

    fn emit_int_mul_high(&mut self, dst: u8, src: u8) {
        self.functions[self.cur_function].push(Instruction::IntMulHigh { dst, src });
    }

    fn emit_int_mul_high_unsigned(&mut self, dst: u8, src: u8) {
        self.functions[self.cur_function].push(Instruction::IntMulHighUnsigned { dst, src });
    }

    fn emit_int_neg(&mut self, dst: u8) {
        self.functions[self.cur_function].push(Instruction::IntNeg { dst });
    }

    fn emit_bit_swap(&mut self, dst: u8, src: u8) {
        self.functions[self.cur_function].push(Instruction::BitSwap { dst, src });
    }

    fn emit_bit_or(&mut self, dst: u8, src: u8) {
        self.functions[self.cur_function].push(Instruction::BitOr { dst, src });
    }

    fn emit_bit_and(&mut self, dst: u8, src: u8) {
        self.functions[self.cur_function].push(Instruction::BitAnd { dst, src });
    }

    fn emit_bit_xor(&mut self, dst: u8, src: u8) {
        self.functions[self.cur_function].push(Instruction::BitXor { dst, src });
    }

    fn emit_bit_shift_left(&mut self, dst: u8, amount: u8) {
        self.functions[self.cur_function].push(Instruction::BitShiftLeft { dst, amount });
    }

    fn emit_bit_shift_right(&mut self, dst: u8, amount: u8) {
        self.functions[self.cur_function].push(Instruction::BitShiftRight { dst, amount });
    }

    fn emit_bit_rotate_left(&mut self, dst: u8, amount: u8) {
        self.functions[self.cur_function].push(Instruction::BitRotateLeft { dst, amount });
    }

    fn emit_bit_rotate_right(&mut self, dst: u8, amount: u8) {
        self.functions[self.cur_function].push(Instruction::BitRotateRight { dst, amount });
    }

    fn emit_cond_branch(&mut self, a: u8, b: u8, params: BranchParams) {
        self.functions[self.cur_function].push(Instruction::CondBranch { a, b, params });
    }

    fn emit_mem_load(&mut self, dst: u8, addr: usize) {
        self.functions[self.cur_function].push(Instruction::MemLoad { dst, addr });
    }

    fn emit_mem_store(&mut self, addr: usize, src: u8) {
        self.functions[self.cur_function].push(Instruction::MemStore { addr, src });
    }

    fn begin_function(&mut self, idx: usize) {
        self.cur_function = idx;
    }

    fn finish(mut self, memory: Vec<i64>) -> Self::Runner {
        self.memory = Some(memory);
        self
    }
}

impl Runner for Interpreter {
    fn step(&mut self) {
        let memory = self.memory.as_mut().expect("memory not initialized");
        Self::call_function(&self.functions, memory, 0);
    }

    fn memory(&self) -> &[i64] {
        self.memory
            .as_ref()
            .expect("memory not initialized")
            .as_ref()
    }

    fn memory_mut(&mut self) -> &mut [i64] {
        self.memory
            .as_mut()
            .expect("memory not initialized")
            .as_mut()
    }
}

impl Interpreter {
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
