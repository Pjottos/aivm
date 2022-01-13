mod interpreter;

pub use interpreter::Interpreter;

pub(crate) use private::CodeGeneratorImpl;

pub trait CodeGenerator: CodeGeneratorImpl {}
impl<G: CodeGeneratorImpl> CodeGenerator for G {}

mod private {
    use crate::{compile::BranchParams, Runner};

    pub trait CodeGeneratorImpl {
        type Runner: Runner;

        fn create(function_count: usize) -> Self;

        fn emit_call(&mut self, idx: usize);
        fn emit_nop(&mut self);

        fn emit_int_add(&mut self, dst: u8, src: u8);
        fn emit_int_sub(&mut self, dst: u8, src: u8);
        fn emit_int_mul(&mut self, dst: u8, src: u8);
        fn emit_int_mul_high(&mut self, dst: u8, src: u8);
        fn emit_int_mul_high_unsigned(&mut self, dst: u8, src: u8);
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
        fn finish(self, memory: Vec<i64>) -> Self::Runner;
    }
}
