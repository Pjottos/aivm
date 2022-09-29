use super::TargetInterface;

use dynasmrt::{
    dynasm,
    x64::{Rq, X64Relocation},
    DynasmApi,
};

pub struct Target {}

impl TargetInterface for Target {
    type Relocation = X64Relocation;

    const MAX_INSTRUCTION_REGS: usize = 4;
    const REGISTER_COUNT: usize = REGISTERS.len();

    fn emit_prologue<A: DynasmApi>(ops: &mut A, stack_size: u32, used_regs_mask: u64) {
        for reg in REGISTERS
            .into_iter()
            .enumerate()
            .filter_map(|(r, reg)| (used_regs_mask & (1 << r) != 0).then_some(reg))
        {
            dynasm!(ops; push Rq(reg));
        }

        if stack_size != 0 {
            dynasm!(ops; sub rsp, WORD (stack_size * 8) as _);
        }
    }

    fn emit_epilogue<A: DynasmApi>(ops: &mut A, stack_size: u32, used_regs_mask: u64) {
        if stack_size != 0 {
            dynasm!(ops
                ; add rsp, WORD (stack_size * 8) as _
            );
        }

        for reg in REGISTERS
            .into_iter()
            .enumerate()
            .rev()
            .filter_map(|(r, reg)| (used_regs_mask & (1 << r) != 0).then_some(reg))
        {
            dynasm!(ops; pop Rq(reg));
        }

        dynasm!(ops; ret);
    }
}

// TODO: use rax and rdx, they need special handling because of the MulHigh instructions
const REGISTERS: [u8; 12] = [
    Rq::R15 as u8,
    Rq::R14 as u8,
    Rq::R13 as u8,
    Rq::R12 as u8,
    Rq::R11 as u8,
    Rq::R10 as u8,
    Rq::R9 as u8,
    Rq::R8 as u8,
    Rq::RBP as u8,
    Rq::RSI as u8,
    Rq::RCX as u8,
    Rq::RBX as u8,
];
