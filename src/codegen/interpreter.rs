use crate::{codegen::CodeGenerator, Runner};

#[derive(Debug, Clone, Copy)]
enum Instruction {
    Call(usize),
    Return,
}

pub struct Interpreter {
    functions: Vec<Vec<Instruction>>,
    memory: Vec<u64>,
    cur_function: usize,
}

impl CodeGenerator for Interpreter {
    type Runner = Self;

    fn create(function_count: usize, mem_size: usize) -> Self {
        Self {
            functions: vec![vec![]; function_count],
            memory: vec![0; mem_size],
            cur_function: 0,
        }
    }

    fn emit_call(&mut self, idx: usize) {
        assert!(
            idx < self.functions.len(),
            "tried to emit call instruction for non-existent function"
        );
        self.functions[self.cur_function].push(Instruction::Call(idx));
    }

    fn emit_return(&mut self) {
        self.functions[self.cur_function].push(Instruction::Return);
    }

    fn next_function(&mut self) {
        assert!(
            self.cur_function + 1 < self.functions.len(),
            "next_function called while already at last"
        );
        self.cur_function += 1;
    }

    fn finish(self) -> Self::Runner {
        self
    }
}

impl Runner for Interpreter {
    fn step(&mut self) {
        Self::call_function(&self.functions, &mut self.memory, 0);
    }

    fn memory(&self) -> &[u64] {
        &self.memory
    }

    fn memory_mut(&mut self) -> &mut [u64] {
        &mut self.memory
    }
}

impl Interpreter {
    fn call_function(functions: &[Vec<Instruction>], memory: &mut [u64], idx: usize) {
        use Instruction::*;

        for instruction in functions[idx].iter().copied() {
            match instruction {
                Call(idx) => Self::call_function(functions, memory, idx),
                Return => break,
            }
        }
    }
}
