use crate::Runner;

mod interpreter;

pub use interpreter::Interpreter;

pub trait CodeGenerator {
    type Runner: Runner;

    fn create(function_count: usize, mem_size: usize) -> Self;

    fn emit_call(&mut self, idx: usize);
    fn emit_return(&mut self);

    fn next_function(&mut self);
    fn finish(self) -> Self::Runner;
}
