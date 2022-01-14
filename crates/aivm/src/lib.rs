#![feature(generic_associated_types)]

pub mod codegen;
mod compile;

pub use compile::Compiler;

pub trait Runner {
    fn step(&mut self);
    fn memory(&self) -> &[i64];
    fn memory_mut(&mut self) -> &mut [i64];
}
