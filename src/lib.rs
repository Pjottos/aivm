mod codegen;
pub mod compile;

pub trait Runner {
    fn step(&mut self);
    fn memory(&self) -> &[u64];
    fn memory_mut(&mut self) -> &mut [u64];
}
