use crate::codegen;

mod ir;

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

    fn finish(&mut self, _memory_size: u32, _input_size: u32, _output_size: u32) -> Self::Runner {
        panic!();
        self.functions.clear();
        Runner
    }
}

impl LightJit {
    /// Create a new generator.
    pub fn new() -> Self {
        Self::default()
    }
}

pub struct Runner;

impl crate::Runner for Runner {
    fn step(&self, _memory: &mut [i64]) {}
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
