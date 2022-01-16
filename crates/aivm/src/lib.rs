#![feature(generic_associated_types)]
#![deny(missing_docs)]
#![cfg_attr(doc_cfg, feature(doc_cfg))]

//! Artificial intelligence that embraces the hardware it runs on.
//!
//! Instead of relying on huge matrix multiplications and non-linear activation functions,
//! `AIVM` uses a virtual machine with trainable code to directly drive its decision making. The
//! code can be compiled into native machine code, removing an expensive layer of abstraction from
//! typical artificial intelligence agents.
//!
//! ## Quick start
//! ```
//! use aivm::{codegen, Compiler, Runner};
//!
//! let gen = codegen::Interpreter::new();
//! let mut compiler = Compiler::new(gen);
//!
//! // TODO: train code and memory to make it do something useful.
//! let code = [0; 16];
//! let memory = vec![0; 4];
//! let mut runner = compiler.compile(&code, memory);
//!
//! runner.step();
//! ```

/// The different code generators available.
pub mod codegen;
mod compile;
mod frequency;

pub use compile::Compiler;
pub use frequency::{DefaultFrequencies, InstructionFrequencies};

/// Returned by a code generator to run VM code.
pub trait Runner {
    /// Run the VM code, calling into the main function once.
    fn step(&mut self);

    /// Returns a reference to the VM's memory. Can be used to read output.
    fn memory(&self) -> &[i64];

    /// Returns a mutable reference to the VM's memory. Can be used to write input.
    fn memory_mut(&mut self) -> &mut [i64];
}
