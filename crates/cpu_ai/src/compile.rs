use crate::{
    codegen::{self, CodeGenKind, CodeGenerator},
    Runner,
};

use core::num::Wrapping;
use std::collections::HashSet;

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct BranchParams(u32);

impl BranchParams {
    #[inline]
    pub fn compare_kind(self) -> CompareKind {
        use CompareKind::*;

        match self.0 >> 30 {
            0 => Eq,
            1 => Neq,
            2 => Gt,
            _ => Lt,
        }
    }

    #[inline]
    pub fn offset(self) -> u32 {
        self.0 & !(3 << 30)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CompareKind {
    Eq,
    Neq,
    Gt,
    Lt,
}

const FREQ_END_FUNC: u16 = 3449;
const FREQ_CALL: u16 = 3449;

const FREQ_INT_ADD: u16 = 3449;
const FREQ_INT_SUB: u16 = 3449;
const FREQ_INT_MUL: u16 = 3453;
const FREQ_INT_MUL_HIGH: u16 = 3449;
const FREQ_INT_MUL_HIGH_SIGNED: u16 = 3449;
const FREQ_INT_NEG: u16 = 3449;

const FREQ_BIT_SWAP: u16 = 3449;
const FREQ_BIT_OR: u16 = 3449;
const FREQ_BIT_AND: u16 = 3449;
const FREQ_BIT_XOR: u16 = 3449;
const FREQ_BIT_SHIFT_L: u16 = 3449;
const FREQ_BIT_SHIFT_R: u16 = 3449;
const FREQ_BIT_ROT_L: u16 = 3449;
const FREQ_BIT_ROT_R: u16 = 3449;

const FREQ_COND_BRANCH: u16 = 3449;

const FREQ_MEM_LOAD: u16 = 3449;
const FREQ_MEM_STORE: u16 = 3449;

pub fn compile_program(
    code: &[u64],
    memory: Vec<Wrapping<i64>>,
    code_gen_kind: CodeGenKind,
) -> Box<dyn Runner> {
    match code_gen_kind {
        CodeGenKind::Interpreter => Box::new(compile::<codegen::Interpreter>(code, memory)),
    }
}

fn compile<G: CodeGenerator>(code: &[u64], memory: Vec<Wrapping<i64>>) -> G::Runner {
    // Count the amount of functions and how many instructions they contain.
    let mut funcs = vec![Function::new(0)];
    for (i, instruction) in code.iter().copied().enumerate() {
        let (kind, _, _, _) = instruction_components(instruction);

        if kind <= FREQ_END_FUNC {
            funcs.push(Function::new(i + 1));
            continue;
        }

        funcs.last_mut().unwrap().instruction_count += 1;
    }

    let func_count = funcs.len();
    let memory_size = memory.len();
    let mut gen = G::create(func_count, memory);

    let mut call_stack = vec![];
    let mut remaining_funcs = vec![(0, 0)];
    let mut compiled_funcs = HashSet::new();

    'funcs: while let Some((f, offset, func)) = remaining_funcs
        .pop()
        .map(|(f, offset)| (f, offset, &mut funcs[f]))
    {
        let is_compiled = compiled_funcs.contains(&f);
        if !is_compiled {
            gen.begin_function(f);
        }

        let start = func.first_instruction + offset;
        let end = func.first_instruction + func.instruction_count;
        for (i, instruction) in code[start..end]
            .iter()
            .copied()
            .enumerate()
            .map(|(i, inst)| (i + offset, inst))
        {
            let (mut kind, dst, src, imm) = instruction_components(instruction);

            // Never included in the function body.
            kind -= FREQ_END_FUNC;

            if cmp_freq(&mut kind, FREQ_CALL) {
                let idx = calc_call_idx(imm, src, dst, func_count);

                // Only emit call instruction when it will not lead to a cycle.
                if !call_stack.contains(&idx) && idx != f {
                    call_stack.push(f);
                    remaining_funcs.push((f, i + 1));
                    remaining_funcs.push((idx, 0));

                    if !is_compiled {
                        gen.emit_call(idx);
                    }

                    continue 'funcs;
                }

                // Ensure instruction count remains the same, for branches.
                if !is_compiled {
                    gen.emit_nop();
                }
            } else if cmp_freq(&mut kind, FREQ_INT_ADD) && !is_compiled {
                gen.emit_int_add(dst, src);
            } else if cmp_freq(&mut kind, FREQ_INT_SUB) && !is_compiled {
                gen.emit_int_sub(dst, src);
            } else if cmp_freq(&mut kind, FREQ_INT_MUL) && !is_compiled {
                gen.emit_int_mul(dst, src);
            } else if cmp_freq(&mut kind, FREQ_INT_MUL_HIGH) && !is_compiled {
                gen.emit_int_mul_high(dst, src);
            } else if cmp_freq(&mut kind, FREQ_INT_MUL_HIGH_SIGNED) && !is_compiled {
                gen.emit_int_mul_high_signed(dst, src);
            } else if cmp_freq(&mut kind, FREQ_INT_NEG) && !is_compiled {
                gen.emit_int_neg(dst);
            } else if cmp_freq(&mut kind, FREQ_BIT_SWAP) && !is_compiled {
                gen.emit_bit_swap(dst, src);
            } else if cmp_freq(&mut kind, FREQ_BIT_OR) && !is_compiled {
                gen.emit_bit_or(dst, src);
            } else if cmp_freq(&mut kind, FREQ_BIT_AND) && !is_compiled {
                gen.emit_bit_and(dst, src);
            } else if cmp_freq(&mut kind, FREQ_BIT_XOR) && !is_compiled {
                gen.emit_bit_xor(dst, src);
            } else if cmp_freq(&mut kind, FREQ_BIT_SHIFT_L) && !is_compiled {
                gen.emit_bit_shift_left(dst, imm as u8);
            } else if cmp_freq(&mut kind, FREQ_BIT_SHIFT_R) && !is_compiled {
                gen.emit_bit_shift_right(dst, imm as u8);
            } else if cmp_freq(&mut kind, FREQ_BIT_ROT_L) && !is_compiled {
                gen.emit_bit_rotate_left(dst, imm as u8);
            } else if cmp_freq(&mut kind, FREQ_BIT_ROT_R) && !is_compiled {
                gen.emit_bit_rotate_right(dst, imm as u8);
            } else if cmp_freq(&mut kind, FREQ_COND_BRANCH) && !is_compiled {
                let max_offset = func.instruction_count - i - 1;
                if max_offset > 0 {
                    let raw_params = (imm & !(3 << 30)) | (imm % max_offset as u32);
                    gen.emit_cond_branch(dst, src, BranchParams(raw_params));
                } else {
                    gen.emit_nop();
                }
            } else if cmp_freq(&mut kind, FREQ_MEM_LOAD) && !is_compiled {
                let addr = imm as usize % memory_size;
                gen.emit_mem_load(dst, addr);
            } else if cmp_freq(&mut kind, FREQ_MEM_STORE) && !is_compiled {
                let addr = imm as usize % memory_size;
                gen.emit_mem_store(addr, src);
            } else if !is_compiled {
                unreachable!("instruction frequencies don't add up to u16::MAX")
            }
        }

        call_stack.pop();

        if !is_compiled {
            compiled_funcs.insert(f);
        }
    }

    gen.finish()
}

#[inline]
fn instruction_components(instruction: u64) -> (u16, u8, u8, u32) {
    let kind = instruction as u16;
    let dst = (instruction >> 16) as u8;
    let src = (instruction >> 24) as u8;
    let imm = (instruction >> 32) as u32;

    (kind, dst, src, imm)
}

#[inline]
fn calc_call_idx(imm: u32, src: u8, dst: u8, func_count: usize) -> usize {
    (imm as usize | ((src as usize) >> 32) | ((dst as usize) >> 40)) % func_count
}

#[inline]
fn cmp_freq(kind: &mut u16, freq: u16) -> bool {
    if *kind <= freq {
        true
    } else {
        *kind -= freq;
        false
    }
}

struct Function {
    first_instruction: usize,
    instruction_count: usize,
}

impl Function {
    fn new(first_instruction: usize) -> Self {
        Self {
            first_instruction,
            instruction_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;

    #[test]
    fn sample() {
        let mut code = [0; 8];
        let memory = vec![Wrapping(0); 128];

        thread_rng().fill(&mut code);

        let mut runner = compile_program(&code, memory, CodeGenKind::default());
        runner.step();
    }
}
