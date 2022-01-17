use crate::{codegen, compile::CompareKind};

use cranelift::{
    codegen::{
        binemit::{NullStackMapSink, NullTrapSink},
        ir,
        settings::{self, Configurable},
        Context,
    },
    frontend::{FunctionBuilder, FunctionBuilderContext, Variable},
    prelude::*,
};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, FuncId, Linkage, Module};

use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    mem,
    num::NonZeroU32,
};

const VAR_MEM_START: u32 = 256;
/// Temporary, for use in the swap instruction.
const VAR_TMP: u32 = 257;

/// A code generator that uses cranelift to JIT compile AIVM code into native machine code.
pub struct Cranelift {
    func_ctx: FunctionBuilderContext,
    func_refs: HashMap<u32, ir::entities::FuncRef>,
    functions: Vec<FuncId>,
    upcoming_blocks: HashMap<u32, Block>,
    module: JITModule,
    ctx: Context,
    cur_function: Option<u32>,
}

impl codegen::private::CodeGeneratorImpl for Cranelift {
    type Runner = Runner;
    type Emitter<'a> = Emitter<'a>;

    fn begin(&mut self, function_count: NonZeroU32) {
        let function_count = function_count.get();

        self.cur_function = None;
        self.functions.clear();
        self.functions.reserve(function_count.try_into().unwrap());

        let sig = self.make_signature();
        let main_func = self
            .module
            .declare_function("main", Linkage::Export, &sig)
            .unwrap();
        self.functions.push(main_func);

        for i in 1..function_count {
            let func = self
                .module
                .declare_function(&i.to_string(), Linkage::Local, &sig)
                .unwrap();
            self.functions.push(func);
        }
    }

    fn begin_function(&mut self, idx: u32) -> Self::Emitter<'_> {
        self.define_cur_function();
        self.cur_function = Some(idx);

        self.func_refs.clear();
        self.upcoming_blocks.clear();
        self.module.clear_context(&mut self.ctx);

        self.ctx.func.signature = self.make_signature();
        self.ctx.func.name =
            ExternalName::user(0, self.functions[usize::try_from(idx).unwrap()].as_u32());

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.func_ctx);

        for i in 0..256 {
            builder.declare_var(Variable::with_u32(i), ir::types::I64);
        }
        builder.declare_var(Variable::with_u32(VAR_MEM_START), ir::types::R64);
        builder.declare_var(Variable::with_u32(VAR_TMP), ir::types::I64);

        let main_block = builder.create_block();
        builder.append_block_params_for_function_params(main_block);
        builder.seal_block(main_block);
        builder.switch_to_block(main_block);

        let mem_start = builder.block_params(main_block)[0];
        builder.def_var(Variable::with_u32(VAR_MEM_START), mem_start);

        Emitter {
            builder,
            func_refs: &mut self.func_refs,
            module: &mut self.module,
            functions: &self.functions,

            upcoming_blocks: &mut self.upcoming_blocks,
            next_instruction: 0,
        }
    }

    fn finish(&mut self, memory_size: u32) -> Self::Runner {
        self.define_cur_function();
        self.module.finalize_definitions();

        let mut module = Self::create_jit_module();
        mem::swap(&mut module, &mut self.module);
        self.module.clear_context(&mut self.ctx);

        Runner {
            func_id: self.functions[0],
            module,
            memory_size: memory_size.try_into().unwrap(),
        }
    }
}

impl Cranelift {
    /// Create a new generator.
    pub fn new() -> Self {
        let module = Self::create_jit_module();
        let ctx = module.make_context();

        Self {
            func_ctx: FunctionBuilderContext::new(),
            func_refs: HashMap::new(),
            functions: vec![],
            upcoming_blocks: HashMap::new(),
            module,
            ctx,
            cur_function: None,
        }
    }

    fn make_signature(&self) -> Signature {
        let mut sig = self.module.make_signature();
        sig.params.push(ir::AbiParam::new(ir::types::R64));

        sig
    }

    fn define_cur_function(&mut self) {
        if let Some(f) = self.cur_function {
            self.module
                .define_function(
                    self.functions[usize::try_from(f).unwrap()],
                    &mut self.ctx,
                    &mut NullTrapSink {},
                    &mut NullStackMapSink {},
                )
                .unwrap();
        }
    }

    fn create_jit_module() -> JITModule {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        // FIXME set back to true once the x64 backend supports it.
        flag_builder.set("is_pic", "false").unwrap();

        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("unsupported host machine: {msg}");
        });
        let isa = isa_builder.finish(settings::Flags::new(flag_builder));
        JITModule::new(JITBuilder::with_isa(isa, default_libcall_names()))
    }
}

impl Default for Cranelift {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Emitter<'a> {
    builder: FunctionBuilder<'a>,
    func_refs: &'a mut HashMap<u32, ir::entities::FuncRef>,
    module: &'a mut JITModule,
    functions: &'a [FuncId],

    upcoming_blocks: &'a mut HashMap<u32, Block>,
    next_instruction: u32,
}

impl<'a> codegen::private::Emitter for Emitter<'a> {
    fn prepare_emit(&mut self) {
        if let Some(block) = self.upcoming_blocks.remove(&self.next_instruction) {
            self.builder.ins().jump(block, &[]);
            self.builder.seal_block(block);
            self.builder.switch_to_block(block);
        }

        self.next_instruction += 1;
    }

    fn finalize(&mut self) {
        self.builder.ins().return_(&[]);
        self.builder.finalize();
    }

    fn emit_call(&mut self, idx: u32) {
        let func_ref = *self.func_refs.entry(idx).or_insert_with(|| {
            self.module.declare_func_in_func(
                self.functions[usize::try_from(idx).unwrap()],
                self.builder.func,
            )
        });

        let mem_start = self.builder.use_var(Variable::with_u32(VAR_MEM_START));
        self.builder.ins().call(func_ref, &[mem_start]);
    }

    fn emit_nop(&mut self) {}

    fn emit_int_add(&mut self, dst: u8, a: u8, b: u8) {
        let a = self.use_var(a);
        let b = self.use_var(b);
        let res = self.builder.ins().iadd(a, b);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_int_sub(&mut self, dst: u8, a: u8, b: u8) {
        let a = self.use_var(a);
        let b = self.use_var(b);
        let res = self.builder.ins().isub(a, b);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_int_mul(&mut self, dst: u8, a: u8, b: u8) {
        let a = self.use_var(a);
        let b = self.use_var(b);
        let res = self.builder.ins().imul(a, b);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_int_mul_high(&mut self, dst: u8, a: u8, b: u8) {
        let a = self.use_var(a);
        let b = self.use_var(b);
        let res = self.builder.ins().smulhi(a, b);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_int_mul_high_unsigned(&mut self, dst: u8, a: u8, b: u8) {
        let a = self.use_var(a);
        let b = self.use_var(b);
        let res = self.builder.ins().umulhi(a, b);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_int_neg(&mut self, dst: u8, src: u8) {
        let src = self.use_var(src);
        let res = self.builder.ins().ineg(src);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_int_abs(&mut self, dst: u8, src: u8) {
        let src = self.use_var(src);

        // FIXME: should use the iabs instruction but the x64 backend does not support it
        let shifted = self.builder.ins().sshr_imm(src, 31);
        let sum = self.builder.ins().iadd(src, shifted);
        let res = self.builder.ins().bxor(sum, shifted);

        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_int_inc(&mut self, dst: u8) {
        let a = self.use_var(dst);
        let res = self.builder.ins().iadd_imm(a, 1);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_int_dec(&mut self, dst: u8) {
        let a = self.use_var(dst);
        let res = self.builder.ins().iadd_imm(a, -1);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_int_min(&mut self, dst: u8, a: u8, b: u8) {
        let a = self.use_var(a);
        let b = self.use_var(b);

        // FIXME: should use the imin instruction but the x64 backend does not support it
        let use_a = self.builder.ins().icmp(IntCC::SignedLessThan, a, b);
        let res = self.builder.ins().select(use_a, a, b);

        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_int_max(&mut self, dst: u8, a: u8, b: u8) {
        let a = self.use_var(a);
        let b = self.use_var(b);

        // FIXME: should use the imax instruction but the x64 backend does not support it
        let use_a = self.builder.ins().icmp(IntCC::SignedLessThan, b, a);
        let res = self.builder.ins().select(use_a, a, b);

        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_bit_swap(&mut self, dst: u8, src: u8) {
        let a = self.use_var(dst);
        let b = self.use_var(src);

        let tmp_var = Variable::with_u32(VAR_TMP);
        self.builder.def_var(tmp_var, a);
        let tmp = self.builder.use_var(tmp_var);

        self.builder.def_var(Self::var(dst), b);
        self.builder.def_var(Self::var(src), tmp);
    }

    fn emit_bit_or(&mut self, dst: u8, a: u8, b: u8) {
        let a = self.use_var(a);
        let b = self.use_var(b);
        let res = self.builder.ins().bor(a, b);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_bit_and(&mut self, dst: u8, a: u8, b: u8) {
        let a = self.use_var(a);
        let b = self.use_var(b);
        let res = self.builder.ins().band(a, b);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_bit_xor(&mut self, dst: u8, a: u8, b: u8) {
        let a = self.use_var(a);
        let b = self.use_var(b);
        let res = self.builder.ins().bxor(a, b);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_bit_not(&mut self, dst: u8, src: u8) {
        let src = self.use_var(src);
        let res = self.builder.ins().bnot(src);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_bit_shift_left(&mut self, dst: u8, src: u8, amount: u8) {
        let a = self.use_var(src);
        let res = self.builder.ins().ishl_imm(a, amount as i64);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_bit_shift_right(&mut self, dst: u8, src: u8, amount: u8) {
        let a = self.use_var(src);
        let res = self.builder.ins().ushr_imm(a, amount as i64);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_bit_rotate_left(&mut self, dst: u8, src: u8, amount: u8) {
        let a = self.use_var(src);
        let res = self.builder.ins().rotl_imm(a, amount as i64);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_bit_rotate_right(&mut self, dst: u8, src: u8, amount: u8) {
        let a = self.use_var(src);
        let res = self.builder.ins().rotr_imm(a, amount as i64);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_bit_select(&mut self, dst: u8, mask: u8, a: u8, b: u8) {
        let mask = self.use_var(mask);
        let a = self.use_var(a);
        let b = self.use_var(b);

        // FIXME: should use the bitselect instruction but the x64 backend does not support it
        let true_bits = self.builder.ins().band(a, mask);
        let mask_not = self.builder.ins().bnot(mask);
        let false_bits = self.builder.ins().band(b, mask_not);
        let res = self.builder.ins().bor(true_bits, false_bits);

        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_bit_popcnt(&mut self, dst: u8, src: u8) {
        let src = self.use_var(src);
        let res = self.builder.ins().popcnt(src);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_bit_reverse(&mut self, dst: u8, src: u8) {
        let src = self.use_var(src);
        let res = self.builder.ins().bitrev(src);
        self.builder.def_var(Self::var(dst), res);
    }

    fn emit_branch_cmp(&mut self, a: u8, b: u8, compare_kind: CompareKind, offset: u32) {
        let x = self.use_var(a);
        let y = self.use_var(b);

        let cond = match compare_kind {
            CompareKind::Eq => IntCC::Equal,
            CompareKind::Neq => IntCC::NotEqual,
            CompareKind::Gt => IntCC::SignedGreaterThan,
            CompareKind::Lt => IntCC::SignedLessThan,
        };
        self.branch_ins(offset, |builder, jump_block| {
            builder.ins().br_icmp(cond, x, y, jump_block, &[])
        });
    }

    fn emit_branch_zero(&mut self, src: u8, offset: u32) {
        let src = self.use_var(src);

        self.branch_ins(offset, |builder, jump_block| {
            builder.ins().brz(src, jump_block, &[])
        });
    }

    fn emit_branch_non_zero(&mut self, src: u8, offset: u32) {
        let src = self.use_var(src);

        self.branch_ins(offset, |builder, jump_block| {
            builder.ins().brnz(src, jump_block, &[])
        });
    }

    fn emit_mem_load(&mut self, dst: u8, addr: u32) {
        let mem_start = self.builder.use_var(Variable::with_u32(VAR_MEM_START));

        let v = self.builder.ins().load(
            ir::types::I64,
            MemFlags::trusted(),
            mem_start,
            addr.checked_mul(8).map(i32::try_from).unwrap().unwrap(),
        );
        self.builder.def_var(Self::var(dst), v);
    }

    fn emit_mem_store(&mut self, addr: u32, src: u8) {
        let v = self.use_var(src);

        let mem_start = self.builder.use_var(Variable::with_u32(VAR_MEM_START));
        self.builder.ins().store(
            MemFlags::trusted(),
            v,
            mem_start,
            addr.checked_mul(8).map(i32::try_from).unwrap().unwrap(),
        );
    }
}

impl<'a> Emitter<'a> {
    fn use_var(&mut self, v: u8) -> ir::entities::Value {
        self.builder.use_var(Self::var(v))
    }

    fn var(v: u8) -> Variable {
        Variable::with_u32(v as u32)
    }

    fn branch_ins<F>(&mut self, offset: u32, instruction_func: F)
    where
        F: FnOnce(&mut FunctionBuilder, Block) -> ir::Inst,
    {
        let resume_block = self.builder.create_block();
        let target_instruction = self.next_instruction - 1 + offset;
        let jump_block = *self
            .upcoming_blocks
            .entry(target_instruction)
            .or_insert_with(|| self.builder.create_block());

        instruction_func(&mut self.builder, jump_block);

        self.builder.ins().jump(resume_block, &[]);
        self.builder.seal_block(resume_block);
        self.builder.switch_to_block(resume_block);
    }
}

pub struct Runner {
    func_id: FuncId,
    module: JITModule,
    memory_size: usize,
}

impl crate::Runner for Runner {
    fn step(&self, memory: &mut [i64]) {
        // It would be unsound to call the compiled code with an invalid pointer.
        assert!(memory.len() >= self.memory_size);

        let ptr = self.module.get_finalized_function(self.func_id);
        let main: fn(*mut i64) = unsafe { mem::transmute(ptr) };

        main(memory.as_mut_ptr());
    }
}

#[cfg(test)]
mod tests {
    //use super::*;

    #[test]
    fn test() {}
}
