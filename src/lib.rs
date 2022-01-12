use core::num::Wrapping;

mod codegen;
pub mod compile;

pub trait Runner {
    fn step(&mut self);
    fn memory(&self) -> &[Wrapping<i64>];
    fn memory_mut(&mut self) -> &mut [Wrapping<i64>];
}
