#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

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
//! const LOWEST_FUNCTION_LEVEL: u32 = 1;
//! const MEMORY_SIZE: u32 = 4;
//! const INPUT_SIZE: u32 = 4;
//! const OUTPUT_SIZE: u32 = 4;
//!
//! let gen = codegen::Interpreter::new();
//! let mut compiler = Compiler::new(gen);
//!
//! // TODO: train code and memory to make it do something useful.
//! let code = [0; 16];
//! let mut runner = compiler.compile(
//!     &code,
//!     LOWEST_FUNCTION_LEVEL,
//!     MEMORY_SIZE,
//!     INPUT_SIZE,
//!     OUTPUT_SIZE,
//! );
//! let mut memory = [0; (MEMORY_SIZE + INPUT_SIZE + OUTPUT_SIZE) as usize];
//!
//! runner.step(&mut memory);
//! ```

/// The different code generators available.
pub mod codegen;
mod compile;
mod frequency;

pub use compile::Compiler;
pub use frequency::{DefaultFrequencies, InstructionFrequencies};

/// Returned by a code generator to run VM code.
pub trait Runner {
    /// Run the VM code, clearing the output and then calling into the main function once.
    ///
    /// The provided memory slice is interpreted as the concatenation of the
    /// memory, input and output in that order. It must be at least as big
    /// as the sum of the sizes that were used while compiling the code.
    fn step(&self, memory: &mut [i64]);
}
