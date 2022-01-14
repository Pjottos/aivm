use crate::{
    codegen::{private::Emitter, CodeGenerator},
    Runner,
};

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
const FREQ_INT_MUL_HIGH_UNSIGNED: u16 = 3449;
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

pub struct Compiler<G: CodeGenerator> {
    gen: G,
    funcs: Vec<Function>,
    remaining_funcs: Vec<(usize, usize)>,
    call_stack: Vec<usize>,
    compiled_funcs: HashSet<usize>,
}

impl<G: CodeGenerator> Compiler<G> {
    pub fn new(gen: G) -> Self {
        Self {
            gen,
            funcs: vec![],
            remaining_funcs: vec![],
            call_stack: vec![],
            compiled_funcs: HashSet::new(),
        }
    }

    pub fn compile(&mut self, code: &[u64], memory: Vec<i64>) -> impl Runner + 'static {
        self.clear();

        // Count the amount of functions and how many instructions they contain.
        self.funcs.push(Function::new(0));
        for (i, instruction) in code.iter().copied().enumerate() {
            let (kind, _, _, _) = instruction_components(instruction);

            if kind <= FREQ_END_FUNC {
                self.funcs.push(Function::new(i + 1));
                continue;
            }

            self.funcs.last_mut().unwrap().instruction_count += 1;
        }

        let func_count = self.funcs.len();
        let memory_size = memory.len();

        self.remaining_funcs.push((0, 0));

        'funcs: while let Some((f, offset, func)) = self
            .remaining_funcs
            .pop()
            .map(|(f, offset)| (f, offset, &self.funcs[f]))
        {
            let mut emitter = self
                .compiled_funcs
                .contains(&f)
                .then(|| self.gen.begin_function(f));

            let start = func.first_instruction + offset;
            let end = func.first_instruction + func.instruction_count;
            for (i, instruction) in code[start..end]
                .iter()
                .copied()
                .enumerate()
                .map(|(i, inst)| (i + offset, inst))
            {
                let (mut kind, dst, src, imm) = instruction_components(instruction);

                if let Some(e) = emitter.as_mut() {
                    e.prepare_emit();
                }

                // Never included in the function body.
                kind -= FREQ_END_FUNC;

                if cmp_freq(&mut kind, FREQ_CALL) {
                    let idx = calc_call_idx(imm, src, dst, func_count);

                    // Only emit call instruction when it will not lead to a cycle.
                    if !self.call_stack.contains(&idx) && idx != f {
                        self.call_stack.push(f);
                        self.remaining_funcs.push((f, i + 1));
                        self.remaining_funcs.push((idx, 0));

                        if let Some(e) = emitter.as_mut() {
                            e.emit_call(idx);
                        }

                        continue 'funcs;
                    }

                    // Ensure instruction count remains the same, for branches.
                    if let Some(e) = emitter.as_mut() {
                        e.emit_nop();
                    }
                } else if let Some(emitter) = emitter.as_mut() {
                    if cmp_freq(&mut kind, FREQ_INT_ADD) {
                        emitter.emit_int_add(dst, src);
                    } else if cmp_freq(&mut kind, FREQ_INT_SUB) {
                        emitter.emit_int_sub(dst, src);
                    } else if cmp_freq(&mut kind, FREQ_INT_MUL) {
                        emitter.emit_int_mul(dst, src);
                    } else if cmp_freq(&mut kind, FREQ_INT_MUL_HIGH) {
                        emitter.emit_int_mul_high(dst, src);
                    } else if cmp_freq(&mut kind, FREQ_INT_MUL_HIGH_UNSIGNED) {
                        emitter.emit_int_mul_high_unsigned(dst, src);
                    } else if cmp_freq(&mut kind, FREQ_INT_NEG) {
                        emitter.emit_int_neg(dst);
                    } else if cmp_freq(&mut kind, FREQ_BIT_SWAP) {
                        emitter.emit_bit_swap(dst, src);
                    } else if cmp_freq(&mut kind, FREQ_BIT_OR) {
                        emitter.emit_bit_or(dst, src);
                    } else if cmp_freq(&mut kind, FREQ_BIT_AND) {
                        emitter.emit_bit_and(dst, src);
                    } else if cmp_freq(&mut kind, FREQ_BIT_XOR) {
                        emitter.emit_bit_xor(dst, src);
                    } else if cmp_freq(&mut kind, FREQ_BIT_SHIFT_L) {
                        emitter.emit_bit_shift_left(dst, imm as u8 & 0x3F);
                    } else if cmp_freq(&mut kind, FREQ_BIT_SHIFT_R) {
                        emitter.emit_bit_shift_right(dst, imm as u8 & 0x3F);
                    } else if cmp_freq(&mut kind, FREQ_BIT_ROT_L) {
                        emitter.emit_bit_rotate_left(dst, imm as u8 & 0x3F);
                    } else if cmp_freq(&mut kind, FREQ_BIT_ROT_R) {
                        emitter.emit_bit_rotate_right(dst, imm as u8 & 0x3F);
                    } else if cmp_freq(&mut kind, FREQ_COND_BRANCH) {
                        let max_offset = func.instruction_count - i - 1;
                        if max_offset > 0 {
                            let raw_params = (imm & !(3 << 30)) | (imm % max_offset as u32);
                            emitter.emit_cond_branch(dst, src, BranchParams(raw_params));
                        } else {
                            emitter.emit_nop();
                        }
                    } else if cmp_freq(&mut kind, FREQ_MEM_LOAD) {
                        let addr = imm as usize % memory_size;
                        emitter.emit_mem_load(dst, addr);
                    } else if cmp_freq(&mut kind, FREQ_MEM_STORE) {
                        let addr = imm as usize % memory_size;
                        emitter.emit_mem_store(addr, src);
                    } else {
                        unreachable!("instruction frequencies don't add up to u16::MAX")
                    }
                }
            }

            self.call_stack.pop();

            if emitter.is_some() {
                self.compiled_funcs.insert(f);
            }
        }

        self.gen.finish(memory)
    }

    fn clear(&mut self) {
        self.funcs.clear();
        self.remaining_funcs.clear();
        self.call_stack.clear();
        self.compiled_funcs.clear();
    }
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
