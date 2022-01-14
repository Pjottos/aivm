use crate::{
    codegen,
    compile::{BranchParams, CompareKind},
};

use cranelift::{
    codegen::{ir, isa},
    frontend::{FunctionBuilder, FunctionBuilderContext},
};

use std::convert::TryInto;

pub struct Cranelift {
    ctx: FunctionBuilderContext,
    cur_function: Option<ir::Function>,
}

impl codegen::private::CodeGeneratorImpl for Cranelift {
    type Runner = Runner;
    type Emitter<'a> = Emitter<'a>;

    fn begin(&mut self, _function_count: usize) {
        self.cur_function = None;
    }

    fn begin_function(&mut self, idx: usize) -> Self::Emitter<'_> {
        self.cur_function = Some(ir::Function::with_name_signature(
            ir::ExternalName::User {
                namespace: 0,
                index: idx.try_into().unwrap(),
            },
            ir::Signature::new(isa::CallConv::Fast),
        ));

        Emitter {
            builder: FunctionBuilder::new(self.cur_function.as_mut().unwrap(), &mut self.ctx),
        }
    }

    fn finish(&mut self, memory: Vec<i64>) -> Self::Runner {
        todo!()
    }
}

impl Cranelift {
    pub fn new() -> Self {
        Self {
            ctx: FunctionBuilderContext::new(),
            cur_function: None,
        }
    }
}

impl Default for Cranelift {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Emitter<'a> {
    builder: FunctionBuilder<'a>,
}

impl<'a> codegen::private::Emitter for Emitter<'a> {
    fn prepare_emit(&mut self) {}

    fn emit_call(&mut self, idx: usize) {
        todo!()
    }

    fn emit_nop(&mut self) {
        todo!()
    }

    fn emit_int_add(&mut self, dst: u8, src: u8) {
        todo!()
    }

    fn emit_int_sub(&mut self, dst: u8, src: u8) {
        todo!()
    }

    fn emit_int_mul(&mut self, dst: u8, src: u8) {
        todo!()
    }

    fn emit_int_mul_high(&mut self, dst: u8, src: u8) {
        todo!()
    }

    fn emit_int_mul_high_unsigned(&mut self, dst: u8, src: u8) {
        todo!()
    }

    fn emit_int_neg(&mut self, dst: u8) {
        todo!()
    }

    fn emit_bit_swap(&mut self, dst: u8, src: u8) {
        todo!()
    }

    fn emit_bit_or(&mut self, dst: u8, src: u8) {
        todo!()
    }

    fn emit_bit_and(&mut self, dst: u8, src: u8) {
        todo!()
    }

    fn emit_bit_xor(&mut self, dst: u8, src: u8) {
        todo!()
    }

    fn emit_bit_shift_left(&mut self, dst: u8, amount: u8) {
        todo!()
    }

    fn emit_bit_shift_right(&mut self, dst: u8, amount: u8) {
        todo!()
    }

    fn emit_bit_rotate_left(&mut self, dst: u8, amount: u8) {
        todo!()
    }

    fn emit_bit_rotate_right(&mut self, dst: u8, amount: u8) {
        todo!()
    }

    fn emit_cond_branch(&mut self, a: u8, b: u8, params: BranchParams) {
        todo!()
    }

    fn emit_mem_load(&mut self, dst: u8, addr: usize) {
        todo!()
    }

    fn emit_mem_store(&mut self, addr: usize, src: u8) {
        todo!()
    }
}

pub struct Runner {
    memory: Vec<i64>,
}

impl crate::Runner for Runner {
    fn step(&mut self) {
        todo!()
    }

    fn memory(&self) -> &[i64] {
        &self.memory
    }

    fn memory_mut(&mut self) -> &mut [i64] {
        &mut self.memory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mut gen = Cranelift::create(1);
    }
}
