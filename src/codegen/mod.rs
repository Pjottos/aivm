use crate::{compile::BranchParams, Runner};

use core::num::Wrapping;

mod interpreter;

pub use interpreter::Interpreter;

pub trait CodeGenerator {
    type Runner: Runner;

    fn create(function_count: usize, memory: Vec<Wrapping<i64>>) -> Self;

    fn emit_call(&mut self, idx: usize);
    fn emit_nop(&mut self);

    fn emit_int_add(&mut self, dst: u8, src: u8);
    fn emit_int_sub(&mut self, dst: u8, src: u8);
    fn emit_int_mul(&mut self, dst: u8, src: u8);
    fn emit_int_mul_high(&mut self, dst: u8, src: u8);
    fn emit_int_mul_high_signed(&mut self, dst: u8, src: u8);
    fn emit_int_neg(&mut self, dst: u8);

    fn emit_bit_swap(&mut self, dst: u8, src: u8);
    fn emit_bit_or(&mut self, dst: u8, src: u8);
    fn emit_bit_and(&mut self, dst: u8, src: u8);
    fn emit_bit_xor(&mut self, dst: u8, src: u8);
    fn emit_bit_shift_left(&mut self, dst: u8, amount: u8);
    fn emit_bit_shift_right(&mut self, dst: u8, amount: u8);
    fn emit_bit_rotate_left(&mut self, dst: u8, amount: u8);
    fn emit_bit_rotate_right(&mut self, dst: u8, amount: u8);

    fn emit_cond_branch(&mut self, a: u8, b: u8, params: BranchParams);

    fn emit_mem_load(&mut self, dst: u8, addr: usize);
    fn emit_mem_store(&mut self, addr: usize, src: u8);

    fn begin_function(&mut self, idx: usize);
    fn finish(self) -> Self::Runner;
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum CodeGenKind {
    Interpreter,
}

impl Default for CodeGenKind {
    fn default() -> Self {
        Self::fastest()
    }
}

impl CodeGenKind {
    pub fn is_supported(self) -> bool {
        match self {
            Self::Interpreter => true,
        }
    }

    pub fn fastest() -> Self {
        Self::Interpreter
    }
}
