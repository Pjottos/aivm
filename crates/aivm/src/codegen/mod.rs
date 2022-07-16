#[cfg(feature = "cranelift")]
mod cranelift;
mod interpreter;

#[cfg(feature = "cranelift")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "cranelift")))]
pub use self::cranelift::Cranelift;
pub use interpreter::Interpreter;

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
        fn finish(&mut self, memory_size: u32) -> Self::Runner;
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

        fn emit_bit_swap(&mut self, dst: u8, src: u8);
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
