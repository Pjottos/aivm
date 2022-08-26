use crate::{
    codegen::{private::Emitter, CodeGenerator},
    DefaultFrequencies, InstructionFrequencies, Runner,
};

use std::num::NonZeroU32;

#[derive(Debug, Clone, Copy)]
pub enum CompareKind {
    Eq,
    Neq,
    Gt,
    Lt,
}

#[derive(Debug, Clone, Copy)]
pub enum MemoryBank {
    Input,
    Output,
    Memory,
}

/// Structure for compiling AIVM code.
///
/// It can be used for multiple compilations to reuse allocations.
pub struct Compiler<G: CodeGenerator> {
    gen: G,
    funcs: Vec<Function>,
}

impl<G: CodeGenerator + 'static> Compiler<G> {
    /// Create a [Compiler] that will use the given code generator.
    pub fn new(gen: G) -> Self {
        Self { gen, funcs: vec![] }
    }

    /// Compile the given code to a runner.
    ///
    /// The parameter `lowest_function_level` controls the lowest (highest value) function
    /// "level" where functions in level `n` can only call functions in levels
    /// `(n, lowest_function_level)`. The entry point is always the only function in level 0, and
    /// other levels .
    ///
    /// # Panics
    /// If `function_levels == u32::MAX`.
    pub fn compile(
        &mut self,
        code: &[u64],
        lowest_function_level: u32,
        input_size: u32,
        output_size: u32,
        memory_size: u32,
    ) -> impl Runner + 'static {
        self.compile_with_frequencies::<DefaultFrequencies>(
            code,
            lowest_function_level,
            input_size,
            output_size,
            memory_size,
        )
    }

    /// Like [compile](Self::compile), but using custom instruction frequencies.
    pub fn compile_with_frequencies<F: InstructionFrequencies>(
        &mut self,
        code: &[u64],
        lowest_function_level: u32,
        input_size: u32,
        output_size: u32,
        memory_size: u32,
    ) -> impl Runner + 'static {
        assert_ne!(lowest_function_level, u32::MAX);

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

        self.funcs.retain(|func| func.instruction_count > 0);
        if self.funcs.is_empty() {
            self.funcs.push(Function::new(0));
        }

        let func_count = u32::try_from(self.funcs.len()).unwrap();
        let (level_size, _last_level_size) = if lowest_function_level == 0 {
            (0, 0)
        } else {
            ceil_div_rem(func_count - 1, lowest_function_level)
        };

        self.gen.begin(NonZeroU32::new(func_count).unwrap());

        for (f, func) in self
            .funcs
            .iter()
            .enumerate()
            .map(|(f, func)| (f as u32, func))
        {
            let cur_level = if f == 0 || level_size == 0 {
                0
            } else {
                1 + (f - 1) / level_size
            };
            let mut emitter = self.gen.begin_function(f);

            let start = func.first_instruction;
            let end = func.first_instruction + usize::try_from(func.instruction_count).unwrap();
            for (i, instruction) in code[start..end].iter().copied().enumerate() {
                let mut kind = instruction as u16;

                let a = (instruction >> 16) as u8 & 0x3f;
                let b = (instruction >> 22) as u8 & 0x3f;
                // 4 bits unused
                let imm = (instruction >> 32) as u32;

                let c = (instruction >> 32) as u8 & 0x3f;
                let d = (instruction >> 46) as u8 & 0x3f;

                emitter.prepare_emit();

                // Never included in the function body.
                kind -= F::END_FUNC;

                if cmp_freq(&mut kind, F::CALL) {
                    if level_size == 0 {
                        // Can never call the entry point
                        emitter.emit_nop();
                    } else {
                        let min_idx = 1 + cur_level * level_size;
                        // Saturating sub to handle the last, potentially partially filled, level
                        let callable_count = func_count.saturating_sub(min_idx);
                        if callable_count == 0 {
                            emitter.emit_nop();
                        } else {
                            let offset = imm % callable_count;
                            emitter.emit_call(min_idx + offset);
                        }
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
                    if memory_size != 0 {
                        let addr = imm % memory_size;
                        emitter.emit_mem_load(MemoryBank::Memory, a, addr);
                    } else {
                        emitter.emit_nop();
                    }
                } else if cmp_freq(&mut kind, F::INPUT_LOAD) {
                    if input_size != 0 {
                        let addr = imm % input_size;
                        emitter.emit_mem_load(MemoryBank::Input, a, addr);
                    } else {
                        emitter.emit_nop();
                    }
                } else if cmp_freq(&mut kind, F::MEM_STORE) {
                    if memory_size != 0 {
                        let addr = imm % memory_size;
                        emitter.emit_mem_store(MemoryBank::Memory, addr, a);
                    } else {
                        emitter.emit_nop();
                    }
                } else if cmp_freq(&mut kind, F::OUTPUT_STORE) {
                    if output_size != 0 {
                        let addr = imm % output_size;
                        emitter.emit_mem_store(MemoryBank::Output, addr, a);
                    } else {
                        emitter.emit_nop();
                    }
                } else {
                    panic!("instruction frequencies don't add up to 65536")
                }
            }

            emitter.finalize();
        }

        self.gen.finish(input_size, output_size, memory_size)
    }

    fn clear(&mut self) {
        self.funcs.clear();
    }
}

#[inline]
fn ceil_div_rem(x: u32, y: u32) -> (u32, u32) {
    let div = x / y;
    let rem = x % y;

    (div + (rem != 0) as u32, rem)
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
        if offset != 0 {
            return Some(offset);
        }
    }

    None
}

struct Function {
    first_instruction: usize,
    instruction_count: u32,
}

impl Function {
    fn new(first_instruction: usize) -> Self {
        Self {
            first_instruction,
            instruction_count: 0,
        }
    }
}
