#[cfg(feature = "cranelift")]
mod cranelift;
mod interpreter;

#[cfg(feature = "cranelift")]
pub use self::cranelift::Cranelift;
pub use interpreter::Interpreter;

pub trait CodeGenerator: private::CodeGeneratorImpl {}

impl<T: private::CodeGeneratorImpl> CodeGenerator for T {}

pub(crate) mod private {
    use crate::{compile::BranchParams, Runner};

    pub trait CodeGeneratorImpl {
        type Runner: Runner + 'static;
        type Emitter<'a>: Emitter + 'a
        where
            Self: 'a;

        fn begin(&mut self, function_count: usize);
        fn begin_function(&mut self, idx: usize) -> Self::Emitter<'_>;
        fn finish(&mut self, memory: Vec<i64>) -> Self::Runner;
    }

    pub trait Emitter {
        fn prepare_emit(&mut self) {}

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
    }
}
