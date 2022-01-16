use crate::{
    codegen::{private::Emitter, CodeGenerator},
    DefaultFrequencies, InstructionFrequencies, Runner,
};

use std::{collections::HashSet, num::NonZeroU32};

#[derive(Debug, Clone, Copy)]
pub enum CompareKind {
    Eq,
    Neq,
    Gt,
    Lt,
}

pub struct Compiler<G: CodeGenerator> {
    gen: G,
    funcs: Vec<Function>,
    remaining_funcs: Vec<(u32, u32)>,
    call_stack: Vec<u32>,
    compile_funcs: HashSet<u32>,
}

impl<G: CodeGenerator + 'static> Compiler<G> {
    pub fn new(gen: G) -> Self {
        Self {
            gen,
            funcs: vec![],
            remaining_funcs: vec![],
            call_stack: vec![],
            compile_funcs: HashSet::new(),
        }
    }

    pub fn compile(&mut self, code: &[u64], memory: Vec<i64>) -> impl Runner + 'static {
        self.compile_with_frequencies::<DefaultFrequencies>(code, memory)
    }

    pub fn compile_with_frequencies<F: InstructionFrequencies>(
        &mut self,
        code: &[u64],
        memory: Vec<i64>,
    ) -> impl Runner + 'static {
        self.clear();

        // Count the amount of functions and how many instructions they contain.
        self.funcs.push(Function::new(0));
        for (i, instruction) in code.iter().copied().enumerate() {
            let kind = instruction as u16;

            if kind < F::END_FUNC {
                self.funcs.push(Function::new(i + 1));
                continue;
            }

            self.funcs.last_mut().unwrap().instruction_count += 1;
        }

        let func_count = u32::try_from(self.funcs.len()).unwrap();
        let memory_size = u32::try_from(memory.len()).unwrap();

        // Detect recursive function calls and prevent them from being emitted.
        // The call that would complete the cycle is blocked.
        self.remaining_funcs.push((0, 0));
        'funcs: while let Some((f, offset, func)) = self
            .remaining_funcs
            .pop()
            .map(|(f, offset)| (f, offset, &mut self.funcs[usize::try_from(f).unwrap()]))
        {
            let start = func.first_instruction + usize::try_from(offset).unwrap();
            let end = func.first_instruction + usize::try_from(func.instruction_count).unwrap();
            for (i, instruction) in code[start..end]
                .iter()
                .copied()
                .enumerate()
                .map(|(i, ins)| (u32::try_from(i).unwrap() + offset, ins))
            {
                let mut kind = instruction as u16;

                kind -= F::END_FUNC;
                if kind < F::CALL {
                    let idx = (instruction >> 32) as u32 % func_count;

                    if idx != f && !self.call_stack.contains(&idx) {
                        self.call_stack.push(f);
                        self.remaining_funcs.push((f, i + 1));
                        self.remaining_funcs.push((idx, 0));

                        continue 'funcs;
                    } else if !func.blocked_calls.contains(&idx) {
                        func.blocked_calls.push(idx);
                    }
                }
            }

            self.compile_funcs.insert(f);
            self.call_stack.pop();
        }

        self.gen.begin(NonZeroU32::new(func_count).unwrap());

        for (f, func) in self
            .compile_funcs
            .iter()
            .copied()
            .map(|f| (f, &self.funcs[usize::try_from(f).unwrap()]))
        {
            let mut emitter = self.gen.begin_function(f);

            let start = func.first_instruction;
            let end = func.first_instruction + usize::try_from(func.instruction_count).unwrap();
            for (i, instruction) in code[start..end].iter().copied().enumerate() {
                let mut kind = instruction as u16;

                let a = (instruction >> 16) as u8;
                let b = (instruction >> 24) as u8;
                let imm = (instruction >> 32) as u32;

                let c = (instruction >> 32) as u8;
                let d = (instruction >> 48) as u8;

                emitter.prepare_emit();

                // Never included in the function body.
                kind -= F::END_FUNC;

                if cmp_freq(&mut kind, F::CALL) {
                    let idx = imm % func_count;

                    if !func.blocked_calls.contains(&idx) {
                        emitter.emit_call(idx);
                    } else {
                        // Ensure instruction count remains the same, for branches.
                        emitter.emit_nop();
                    }
                } else if cmp_freq(&mut kind, F::INT_ADD) {
                    emitter.emit_int_add(a, b, c);
                } else if cmp_freq(&mut kind, F::INT_SUB) {
                    emitter.emit_int_sub(a, b, c);
                } else if cmp_freq(&mut kind, F::INT_MUL) {
                    emitter.emit_int_mul(a, b, c);
                } else if cmp_freq(&mut kind, F::INT_MUL_HIGH) {
                    emitter.emit_int_mul_high(a, b, c);
                } else if cmp_freq(&mut kind, F::INT_MUL_HIGH_UNSIGNED) {
                    emitter.emit_int_mul_high_unsigned(a, b, c);
                } else if cmp_freq(&mut kind, F::INT_NEG) {
                    emitter.emit_int_neg(a, b);
                } else if cmp_freq(&mut kind, F::INT_ABS) {
                    emitter.emit_int_abs(a, b);
                } else if cmp_freq(&mut kind, F::INT_INC) {
                    emitter.emit_int_inc(a);
                } else if cmp_freq(&mut kind, F::INT_DEC) {
                    emitter.emit_int_dec(a);
                } else if cmp_freq(&mut kind, F::INT_MIN) {
                    emitter.emit_int_min(a, b, c);
                } else if cmp_freq(&mut kind, F::INT_MAX) {
                    emitter.emit_int_max(a, b, c);
                } else if cmp_freq(&mut kind, F::BIT_SWAP) {
                    emitter.emit_bit_swap(a, b);
                } else if cmp_freq(&mut kind, F::BIT_OR) {
                    emitter.emit_bit_or(a, b, c);
                } else if cmp_freq(&mut kind, F::BIT_AND) {
                    emitter.emit_bit_and(a, b, c);
                } else if cmp_freq(&mut kind, F::BIT_XOR) {
                    emitter.emit_bit_xor(a, b, c);
                } else if cmp_freq(&mut kind, F::BIT_NOT) {
                    emitter.emit_bit_not(a, b);
                } else if cmp_freq(&mut kind, F::BIT_SHIFT_L) {
                    emitter.emit_bit_shift_left(a, b, c & 0x3F);
                } else if cmp_freq(&mut kind, F::BIT_SHIFT_R) {
                    emitter.emit_bit_shift_right(a, b, c & 0x3F);
                } else if cmp_freq(&mut kind, F::BIT_ROT_L) {
                    emitter.emit_bit_rotate_left(a, b, c & 0x3F);
                } else if cmp_freq(&mut kind, F::BIT_ROT_R) {
                    emitter.emit_bit_rotate_right(a, b, c & 0x3F);
                } else if cmp_freq(&mut kind, F::BIT_SELECT) {
                    emitter.emit_bit_select(a, b, c, d);
                } else if cmp_freq(&mut kind, F::BIT_POPCNT) {
                    emitter.emit_bit_popcnt(a, b);
                } else if cmp_freq(&mut kind, F::BIT_REVERSE) {
                    emitter.emit_bit_reverse(a, b);
                } else if cmp_freq(&mut kind, F::BRANCH_CMP) {
                    if let Some(offset) = branch_offset(imm, func, i as u32) {
                        let compare_kind = match a & 3 {
                            0 => CompareKind::Eq,
                            1 => CompareKind::Neq,
                            2 => CompareKind::Gt,
                            _ => CompareKind::Lt,
                        };

                        emitter.emit_branch_cmp(b, c, compare_kind, offset);
                    } else {
                        emitter.emit_nop();
                    }
                } else if cmp_freq(&mut kind, F::BRANCH_ZERO) {
                    if let Some(offset) = branch_offset(imm, func, i as u32) {
                        emitter.emit_branch_zero(a, offset);
                    } else {
                        emitter.emit_nop();
                    }
                } else if cmp_freq(&mut kind, F::BRANCH_NON_ZERO) {
                    if let Some(offset) = branch_offset(imm, func, i as u32) {
                        emitter.emit_branch_non_zero(a, offset);
                    } else {
                        emitter.emit_nop();
                    }
                } else if cmp_freq(&mut kind, F::MEM_LOAD) {
                    let addr = imm % memory_size;
                    emitter.emit_mem_load(a, addr);
                } else if cmp_freq(&mut kind, F::MEM_STORE) {
                    let addr = imm % memory_size;
                    emitter.emit_mem_store(addr, a);
                } else {
                    unreachable!("instruction frequencies don't add up to u16::MAX")
                }
            }

            emitter.finalize();
        }

        self.gen.finish(memory)
    }

    fn clear(&mut self) {
        self.funcs.clear();
        self.remaining_funcs.clear();
        self.call_stack.clear();
        self.compile_funcs.clear();
    }
}

#[inline]
fn cmp_freq(kind: &mut u16, freq: u16) -> bool {
    if *kind < freq {
        true
    } else {
        *kind -= freq;
        false
    }
}

#[inline]
fn branch_offset(imm: u32, func: &Function, cur_offset: u32) -> Option<u32> {
    let max_offset = func.instruction_count - cur_offset - 1;

    if max_offset > 1 {
        let offset = imm % max_offset;
        if offset > 1 {
            return Some(offset);
        }
    }

    None
}

struct Function {
    first_instruction: usize,
    instruction_count: u32,
    blocked_calls: Vec<u32>,
}

impl Function {
    fn new(first_instruction: usize) -> Self {
        Self {
            first_instruction,
            instruction_count: 0,
            blocked_calls: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;

    #[cfg(feature = "cranelift")]
    #[test]
    fn sample() {
        let mut code = [0; 256];
        thread_rng().fill(&mut code);

        let gen = crate::codegen::Cranelift::new();
        let mut compiler = Compiler::new(gen);
        let mut runner = compiler.compile(&code, vec![0; 128]);

        thread_rng().fill(&mut code);
        let mut runner2 = compiler.compile(&code, vec![0; 128]);

        drop(compiler);

        runner2.step();
        runner.step();
    }
}
