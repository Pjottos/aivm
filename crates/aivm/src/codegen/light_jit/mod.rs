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

    fn finish(&mut self, _input_size: u32, _output_size: u32, _memory_size: u32) -> Self::Runner {
        if self.functions[0].blocks.len() > 1 {
            panic!("{:#?}", self.functions);
        }
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
    fn step(&self, _input: &[i64], _output: &mut [i64], _memory: &mut [i64]) {}
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
        let input = [0; 256];
        let mut output = [0; 128];
        let mut mem = [0; 64];

        let gen = LightJit::new();
        let mut compiler = Compiler::new(gen);
        let runner = compiler.compile(&code, 4, 256, 128, 64);

        drop(compiler);

        runner.step(&input, &mut output, &mut mem);
    }
}
