use crate::{
    codegen::light_jit::{
        arch::TargetInterface,
        ir::InstructionKind,
        regalloc::{PhysicalVar, RegAllocAction, RegAllocInstruction},
    },
    compile::CompareKind,
};

use dynasmrt::{
    dynasm,
    x64::{Rq, X64Relocation},
    DynasmApi, DynasmLabelApi,
};

pub struct Target {}

impl TargetInterface for Target {
    type Relocation = X64Relocation;

    const MAX_INSTRUCTION_REGS: usize = 4;
    const REGISTER_COUNT: usize = REGISTERS.len();

    fn supports_mem_operand(kind: InstructionKind) -> bool {
        use InstructionKind::*;
        matches!(
            kind,
            BranchCmp { .. }
                | IntSub
                | IntMul
                | IntMulHigh
                | IntMulHighUnsigned
                | IntNeg
                | BitOr
                | BitAnd
                | BitXor
                | BitNot
                | BitShiftLeft { .. }
                | BitShiftRight { .. }
                | BitRotateLeft { .. }
                | BitRotateRight { .. }
                | BitSelect
        )
    }

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

    fn emit_instruction<A: DynasmLabelApi<Relocation = Self::Relocation>>(
        ops: &mut A,
        inst: RegAllocInstruction,
        func_labels: &[dynasmrt::DynamicLabel],
        block_labels: &[dynasmrt::DynamicLabel],
    ) {
        use InstructionKind::*;

        let mut branch_exit = None;
        for action in inst.actions {
            match action {
                RegAllocAction::RegToStack(s, r) => {
                    dynasm!(ops; mov [rsp + (s * 8) as i32], Rq(REGISTERS[r as usize]))
                }
                RegAllocAction::StackToReg(r, s) => {
                    dynasm!(ops; mov Rq(REGISTERS[r as usize]), [rsp + (s * 8) as i32])
                }
                RegAllocAction::BlockStart(b) => dynasm!(ops; =>block_labels[b.0 as usize]),
                RegAllocAction::BranchExit(b) => branch_exit = Some(b.0 as usize),
            }
        }

        let d = inst.defs;
        let u = inst.uses;

        macro_rules! dyn_op {
            ($inst:ident $a:ident, $b:expr) => {
                if !$b.is_stack() {
                    dynasm!(ops; $inst $a, Rq(reg($b)));
                } else {
                    dynasm!(ops; $inst $a, [rsp + $b.offset()]);
                }
            };
            ($inst:ident $a:expr, $b:ident) => {
                if !$a.is_stack() {
                    dynasm!(ops; $inst Rq(reg($a)), $b);
                } else {
                    dynasm!(ops; $inst [rsp + $a.offset()], $b);
                }
            };
            ($inst:ident $a:expr) => {
                if !$a.is_stack() {
                    dynasm!(ops; $inst Rq(reg($a)));
                } else {
                    dynasm!(ops; $inst QWORD [rsp + $a.offset()]);
                }
            };
            ($inst:ident $a:expr, $b:expr) => {
                if !$a.is_stack() && !$b.is_stack() {
                    dynasm!(ops; $inst Rq(reg($a)), Rq(reg($b)));
                } else if !$a.is_stack() && $b.is_stack() {
                    dynasm!(ops; $inst Rq(reg($a)), [rsp + $b.offset()]);
                } else if $a.is_stack() && !$b.is_stack() {
                    dynasm!(ops; $inst [rsp + $a.offset()], Rq(reg($b)));
                } else {
                    unreachable!();
                }
            };
        }

        match inst.kind {
            Jump => unreachable!(),
            Return => (),
            InitVar => {
                dynasm!(ops; xor Rq(reg(d[0])), Rq(reg(d[0])));
            }
            Call { idx } => dynasm!(ops; call =>func_labels[idx as usize]),
            BranchCmp { compare_kind } => {
                dyn_op!(cmp u[0], u[1]);
                match compare_kind {
                    CompareKind::Eq => dynasm!(ops; je =>block_labels[branch_exit.unwrap()]),
                    CompareKind::Neq => dynasm!(ops; jne =>block_labels[branch_exit.unwrap()]),
                    CompareKind::Gt => dynasm!(ops; jg =>block_labels[branch_exit.unwrap()]),
                    CompareKind::Lt => dynasm!(ops; jl =>block_labels[branch_exit.unwrap()]),
                }
            }
            BranchZero => dynasm!(ops;
                test Rq(reg(u[0])), Rq(reg(u[0]));
                je =>block_labels[branch_exit.unwrap()]
            ),
            BranchNonZero => dynasm!(ops;
                test Rq(reg(u[0])), Rq(reg(u[0]));
                jne =>block_labels[branch_exit.unwrap()]
            ),
            IntAdd => dynasm!(ops; lea Rq(reg(d[0])), [Rq(reg(u[0])) + Rq(reg(u[0]))]),
            IntSub => {
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0]);
                }
                dyn_op!(sub d[0], u[1]);
            }
            IntMul => {
                if d[0].is_stack() {
                    dyn_op!(mov rax, u[0]);
                    dyn_op!(imul u[1]);
                    dyn_op!(mov d[0], rax);
                } else {
                    dyn_op!(mov d[0], u[0]);
                    if u[1].is_stack() {
                        dynasm!(ops; imul Rq(reg(d[0])), [rsp + u[1].offset()])
                    } else {
                        dynasm!(ops; imul Rq(reg(d[0])), Rq(reg(u[1])))
                    }
                }
            }
            IntMulHigh => {
                dyn_op!(mov rax, u[0]);
                dyn_op!(imul u[1]);
                dyn_op!(mov d[0], rdx);
            }
            IntMulHighUnsigned => {
                dyn_op!(mov rax, u[0]);
                dyn_op!(mul u[1]);
                dyn_op!(mov d[0], rdx);
            }
            IntNeg => {
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0]);
                }
                dyn_op!(neg d[0]);
            }
            IntAbs => {
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0]);
                }
                dyn_op!(neg d[0]);
                if u[0].is_stack() {
                    dynasm!(ops; cmovs Rq(reg(d[0])), [rsp + u[0].offset()]);
                } else {
                    dynasm!(ops; cmovs Rq(reg(d[0])), Rq(reg(u[0])));
                }
            }
            IntInc => {
                dynasm!(ops; lea Rq(reg(d[0])), [Rq(reg(u[0])) + 1])
            }
            IntDec => {
                dynasm!(ops; lea Rq(reg(d[0])), [Rq(reg(u[0])) - 1])
            }
            IntMin => {
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0]);
                }
                dyn_op!(cmp u[0], u[1]);
                if u[1].is_stack() {
                    dynasm!(ops; cmovg Rq(reg(d[0])), [rsp + u[1].offset()]);
                } else {
                    dynasm!(ops; cmovg Rq(reg(d[0])), Rq(reg(u[1])));
                }
            }
            IntMax => {
                if d[0] != u[1] {
                    dyn_op!(mov d[0], u[1]);
                }
                dyn_op!(cmp u[0], u[1]);
                if u[0].is_stack() {
                    dynasm!(ops; cmovg Rq(reg(d[0])), [rsp + u[0].offset()]);
                } else {
                    dynasm!(ops; cmovg Rq(reg(d[0])), Rq(reg(u[0])));
                }
            }
            BitOr => {
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0]);
                }
                dyn_op!(or d[0], u[1]);
            }
            BitAnd => {
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0]);
                }
                dyn_op!(and d[0], u[1]);
            }
            BitXor => {
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0]);
                }
                dyn_op!(xor d[0], u[1]);
            }
            BitNot => {
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0]);
                }
                dyn_op!(not d[0]);
            }
            BitShiftLeft { amount } => {
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0])
                }
                if amount != 0 {
                    if d[0].is_stack() {
                        dynasm!(ops; shl [rsp + d[0].offset()], amount as i8);
                    } else {
                        dynasm!(ops; shl Rq(reg(d[0])), amount as i8);
                    }
                }
            }
            BitShiftRight { amount } => {
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0])
                }
                if amount != 0 {
                    if d[0].is_stack() {
                        dynasm!(ops; sar [rsp + d[0].offset()], amount as i8);
                    } else {
                        dynasm!(ops; sar Rq(reg(d[0])), amount as i8);
                    }
                }
            }
            BitRotateLeft { amount } => {
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0])
                }
                if amount != 0 {
                    if d[0].is_stack() {
                        dynasm!(ops; rol [rsp + d[0].offset()], amount as i8);
                    } else {
                        dynasm!(ops; rol Rq(reg(d[0])), amount as i8);
                    }
                }
            }
            BitRotateRight { amount } => {
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0])
                }
                if amount != 0 {
                    if d[0].is_stack() {
                        dynasm!(ops; ror [rsp + d[0].offset()], amount as i8);
                    } else {
                        dynasm!(ops; ror Rq(reg(d[0])), amount as i8);
                    }
                }
            }
            BitSelect => {
                debug_assert!(d[0] != u[1] && d[0] != u[2]);
                if d[0] != u[0] {
                    dyn_op!(mov d[0], u[0])
                }
                dyn_op!(xor d[0], u[1]);
                dyn_op!(and d[0], u[2]);
                dyn_op!(xor d[0], u[1]);
            }
            BitPopcnt => {
                debug_assert!(!d[0].is_stack());
                if u[0].is_stack() {
                    dynasm!(ops; popcnt Rq(reg(d[0])), [rsp + u[0].offset()]);
                } else {
                    dynasm!(ops; popcnt Rq(reg(d[0])), Rq(reg(u[0])));
                }
            }
            BitReverse => {
                debug_assert!(!d[0].is_stack());
                let dst = reg(d[0]);
                dyn_op!(mov rax, u[0]);
                dynasm!(ops
                    ; bswap rax
                    ; mov rdx, 0x0F0F0F0F0F0F0F0F
                    ; mov Rq(dst), rax
                    ; and rax, rdx
                    ; shr Rq(dst), 4
                    ; shl rax, 4
                    ; and Rq(dst), rdx
                    ; or rax, Rq(dst)
                    ; mov Rq(dst), 0x3333333333333333
                    ; mov rdx, rax
                    ; shr rax, 2
                    ; and rdx, Rq(dst)
                    ; and rax, Rq(dst)
                    ; lea Rq(dst), [rax + 4*rdx]
                    ; mov rdx, 0x5555555555555555
                    ; mov rax, Rq(dst)
                    ; shr Rq(dst), 1
                    ; and rax, rdx
                    ; and Rq(dst), rdx
                    ; lea Rq(dst), [Rq(dst) + 2*rax]
                )
            }
            MemLoad { addr } => {
                debug_assert!(!d[0].is_stack());
                let dst = reg(d[0]);
                dynasm!(ops
                    ; mov Rq(dst), addr as i32 * 8
                    ; mov Rq(dst), [rdi + Rq(dst)]
                );
            }
            MemStore { addr } => {
                debug_assert!(!u[0].is_stack());
                dynasm!(ops
                    ; mov rax, addr as i32 * 8
                    ; mov Rq(reg(u[0])), [rdi + rax]
                );
            }
        }
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

#[inline]
fn reg(v: PhysicalVar) -> u8 {
    REGISTERS[v.idx() as usize]
}
