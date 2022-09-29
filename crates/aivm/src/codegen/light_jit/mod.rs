use crate::codegen::{
    self,
    light_jit::arch::{Target, TargetInterface},
};

use dynasmrt::{dynasm, Assembler, AssemblyOffset, DynasmApi, DynasmLabelApi, ExecutableBuffer};

use std::mem::transmute;

mod arch;
mod ir;
mod regalloc;

/// A code generator that does very minimal optimization and generates machine code.
#[derive(Default)]
pub struct LightJit {
    functions: Vec<ir::Function>,
}

impl codegen::private::CodeGeneratorImpl for LightJit {
    type Emitter<'a> = ir::Emitter<'a>;
    type Runner = Runner;

    fn begin(&mut self, function_count: std::num::NonZeroU32) {
        self.functions
            .resize_with(function_count.get() as usize, Default::default);
    }

    fn begin_function(&mut self, idx: u32) -> Self::Emitter<'_> {
        ir::Emitter::new(&mut self.functions[idx as usize])
    }

    fn finish(&mut self, memory_size: u32, input_size: u32, output_size: u32) -> Self::Runner {
        let mut ops = Assembler::<<Target as TargetInterface>::Relocation>::new().unwrap();
        let func_labels: Vec<_> = (0..self.functions.len())
            .map(|_| ops.new_dynamic_label())
            .collect();

        for (f, func) in self.functions.drain(..).enumerate() {
            let reg_allocs = func.reg_allocs.unwrap();

            dynasm!(ops; =>func_labels[f]);
            Target::emit_prologue(&mut ops, reg_allocs.stack_size, reg_allocs.used_regs_mask);

            // TODO: emit function body

            Target::emit_epilogue(&mut ops, reg_allocs.stack_size, reg_allocs.used_regs_mask);
        }

        let code = ops.finalize().unwrap();

        Runner {
            memory_size,
            input_size,
            output_size,
            code,
        }
    }
}

impl LightJit {
    /// Create a new generator.
    pub fn new() -> Self {
        Self::default()
    }
}

pub struct Runner {
    memory_size: u32,
    input_size: u32,
    output_size: u32,
    code: ExecutableBuffer,
}

impl crate::Runner for Runner {
    fn step(&self, memory: &mut [i64]) {
        assert!((self.memory_size + self.input_size + self.output_size) as usize <= memory.len());

        let output_range = memory.len() - self.output_size as usize..;
        memory[output_range].fill(0);

        //println!("{:02x?}", &self.code[..]);

        let entry: extern "sysv64" fn(*mut i64) =
            unsafe { transmute(self.code.ptr(AssemblyOffset(0))) };
        entry(memory.as_mut_ptr());

        panic!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile::Compiler, Runner};
    use rand::prelude::*;

    #[test]
    fn sample() {
        let mut code = [0; 256];
        thread_rng().fill(&mut code);
        let mut mem = [0; 64 + 256 + 128];

        let gen = LightJit::new();
        let mut compiler = Compiler::new(gen);
        let runner = compiler.compile(&code, 4, 64, 256, 128);

        drop(compiler);

        runner.step(&mut mem);
    }
}
