use std::fmt::Debug;

use bitvec::prelude::*;

use crate::{
    codegen::{self, jit::regalloc::RegAllocations},
    compile::CompareKind,
};

pub struct Emitter<'a> {
    func: &'a mut Function,
    instruction_count: u32,
    branch_targets: Vec<PendingBranchTarget>,
    cur_block: Block,
}

impl<'a> Emitter<'a> {
    pub fn new(func: &'a mut Function) -> Self {
        Self {
            func,
            instruction_count: 0,
            branch_targets: vec![],
            cur_block: Block {
                instructions: (0..64)
                    .map(|i| Instruction {
                        kind: InstructionKind::InitVar,
                        dst: [Var::new(i)],
                        ..Instruction::default()
                    })
                    .collect(),
                var_def_mask: VarMask::ALL,
                ..Block::default()
            },
        }
    }

    fn next_block_name(&self) -> BlockName {
        BlockName(self.func.blocks.len() as u32 + 1)
    }

    fn cur_block_name(&self) -> BlockName {
        BlockName(self.func.blocks.len() as u32)
    }

    fn finish_block(&mut self) {
        let mut block = Block::default();
        std::mem::swap(&mut self.cur_block, &mut block);
        self.func.blocks.push(block);
    }

    fn finish_block_with_branch(&mut self, inst: Instruction, offset: u32) {
        let block_name = self.cur_block_name();
        let fall_through_proxy_block_name = BlockName(block_name.0 + 1);
        let branch_proxy_block_name = BlockName(block_name.0 + 2);
        let next_block_name = BlockName(block_name.0 + 3);

        self.cur_block.instructions.push(inst);
        self.cur_block.exit = fall_through_proxy_block_name;
        self.cur_block.branch_exit = branch_proxy_block_name;
        self.finish_block();

        // Split critical edges, since most blocks have at least 1 predecessor, the previous block, most
        // edges where a branch is taken are critical. Edges where the branch is not taken are
        // potentially critical if the following block is the target of another branch
        // instruction. For simplicity we always generate proxy blocks

        // Fall through proxy
        self.cur_block.instructions.push(Instruction::jump());
        self.cur_block.predecessors.push(block_name);
        self.cur_block.exit = next_block_name;
        self.finish_block();

        // Branch proxy
        self.cur_block.instructions.push(Instruction::jump());
        self.cur_block.predecessors.push(block_name);
        self.finish_block();

        self.cur_block
            .predecessors
            .push(fall_through_proxy_block_name);

        let target_instruction = self.instruction_count + offset;
        self.branch_targets.push(PendingBranchTarget {
            branch_proxy_block_name,
            target_instruction,
        });
    }

    fn finish_block_with_fall_through(&mut self) {
        let block_name = self.cur_block_name();
        self.cur_block.instructions.push(Instruction::jump());
        self.cur_block.exit = self.next_block_name();
        self.finish_block();
        self.cur_block.predecessors.push(block_name);
    }

    fn create_branch_targets(&mut self) {
        // Use `drain_filter` when stabilized (https://github.com/rust-lang/rust/issues/43244)
        let mut i = 0;
        while i < self.branch_targets.len() {
            if self.branch_targets[i].target_instruction == self.instruction_count {
                let PendingBranchTarget {
                    branch_proxy_block_name,
                    ..
                } = self.branch_targets.swap_remove(i);

                // Begin new block for branch to jump to
                if !self.cur_block.instructions.is_empty() {
                    self.finish_block_with_fall_through();
                }

                self.cur_block.predecessors.push(branch_proxy_block_name);
                self.func.blocks[branch_proxy_block_name.0 as usize].exit = self.cur_block_name();
            } else {
                i += 1;
            }
        }
    }

    fn def_var(&mut self, name: u8) -> Var {
        self.cur_block.var_def_mask.insert(name);
        Var::new(name)
    }

    fn use_var(&self, name: u8) -> Var {
        Var::new(name)
    }
}

impl<'a> codegen::private::Emitter for Emitter<'a> {
    fn prepare_emit(&mut self) {
        self.create_branch_targets();
        self.instruction_count += 1;
    }

    fn finalize(&mut self) {
        self.create_branch_targets();

        self.cur_block.instructions.push(Instruction::return_());
        self.finish_block();

        // Initialize dominators array
        // The blocks array is naturally in reverse post order
        let mut doms = vec![BlockName::INVALID; self.func.blocks.len()];
        doms[0] = BlockName(0);
        let mut changed = true;
        while changed {
            changed = false;

            for (b, block) in self.func.blocks.iter().enumerate().skip(1) {
                let mut new_idom = block
                    .predecessors
                    .iter()
                    .copied()
                    .find(|p| doms[p.0 as usize].is_valid())
                    .unwrap();
                let initial_idom = new_idom;

                for predecessor in block
                    .predecessors
                    .iter()
                    .copied()
                    .filter(|&p| p != initial_idom)
                {
                    if doms[predecessor.0 as usize].is_valid() {
                        let mut finger1 = predecessor.0;
                        let mut finger2 = new_idom.0;

                        while finger1 != finger2 {
                            while finger1 > finger2 {
                                finger1 = doms[finger1 as usize].0;
                            }
                            while finger2 > finger1 {
                                finger2 = doms[finger2 as usize].0;
                            }
                        }

                        new_idom = BlockName(finger1);
                    }
                }

                changed = doms[b] != new_idom;
                doms[b] = new_idom;
            }
        }

        // Build dominance frontier sets
        let mut dominance_frontiers = vec![vec![]; self.func.blocks.len()];
        for (b, block) in self
            .func
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| b.predecessors.len() > 1)
        {
            let b = BlockName(b as u32);
            for p in block.predecessors.iter().copied() {
                let mut runner = p;
                while runner != doms[b.0 as usize] {
                    let dominance_frontier = &mut dominance_frontiers[runner.0 as usize];
                    if !dominance_frontier.contains(&b) {
                        dominance_frontier.push(b);
                    }
                    runner = doms[runner.0 as usize];
                }
            }
        }

        // Insert block params where necessary
        let mut processed_blocks = bitvec![0; self.func.blocks.len()];
        let mut pushed_blocks = bitvec![0; self.func.blocks.len()];
        let mut block_stack = vec![];
        for v in 0..64 {
            processed_blocks.set_elements(0);
            pushed_blocks.set_elements(0);
            block_stack.clear();

            for b in self
                .func
                .blocks
                .iter()
                .enumerate()
                .filter_map(|(b, block)| {
                    block
                        .var_def_mask
                        .contains(v)
                        .then_some(BlockName(b as u32))
                })
            {
                block_stack.push(b);
                pushed_blocks.set(b.0 as usize, true);
            }

            while let Some(b) = block_stack.pop() {
                for &f in &dominance_frontiers[b.0 as usize] {
                    if !processed_blocks[f.0 as usize] {
                        self.func.blocks[f.0 as usize].params.push(Var::new(v));
                        self.func.blocks[f.0 as usize].var_def_mask.insert(v);
                        processed_blocks.set(f.0 as usize, true);

                        if !pushed_blocks[f.0 as usize] {
                            pushed_blocks.set(f.0 as usize, true);
                            block_stack.push(f);
                        }
                    }
                }
            }
        }

        let mut version_counters = [0; 64];
        // Should be a stack array but Vec doesn't implement Copy
        let mut var_stacks = vec![vec![]; 64];
        let mut block_stack = vec![];
        let mut live_ranges = vec![];

        let mut gen_name =
            |v: &mut Var, var_stacks: &mut [Vec<(u32, u32, u32)>], cur_instruction: u32| {
                let counter = &mut version_counters[v.name() as usize];
                v.set_version(*counter);
                var_stacks[v.name() as usize].push((*counter, cur_instruction, 0));
                *counter += 1;
            };

        block_stack.push((BlockName(0), BlockName(0)));
        while let Some((b, last_child)) = block_stack.pop() {
            let instructions_start = self
                .func
                .blocks
                .iter()
                .take(b.0 as usize)
                .map(|b| b.instructions.len() as u32)
                .sum();
            let block = &mut self.func.blocks[b.0 as usize];
            if b == last_child {
                for var in &mut block.params {
                    gen_name(var, &mut var_stacks, instructions_start);
                }

                for (i, inst) in (instructions_start..).zip(block.instructions.iter_mut()) {
                    for src in inst.src_iter_mut() {
                        let stack_entry = var_stacks[src.name() as usize].last_mut().unwrap();
                        // Update the live interval to include the current latest usage
                        debug_assert!(i + 1 >= stack_entry.2);
                        stack_entry.2 = i + 1;
                        src.set_version(stack_entry.0);
                    }
                    for dst in inst.dst_iter_mut() {
                        gen_name(dst, &mut var_stacks, i);
                    }
                }
            }

            // Visit children in dominator tree
            if let Some(child) = doms
                .iter()
                .copied()
                .enumerate()
                .skip(1 + last_child.0 as usize)
                .find_map(|(c, p)| (p == b).then_some(BlockName(c as u32)))
            {
                block_stack.push((b, child));
                block_stack.push((child, child));
                continue;
            }

            // Pop from stack in reverse order to match var versions
            // Since the same variable name cannot appear twice in either the instruction
            // destinations or the block parameters, we don't have to reverse those
            for var in block
                .instructions
                .iter()
                .rev()
                .flat_map(|inst| inst.dst_iter())
                .chain(block.params.iter().copied())
            {
                let (version, start, end) = var_stacks[var.name() as usize].pop().unwrap();
                debug_assert_eq!(var.version(), version);

                live_ranges.push(LiveRange { var, start, end });
            }
        }

        live_ranges.sort_unstable_by_key(|r| if r.end == 0 { u32::MAX } else { r.start });
        // Don't need variables that never get read
        if let Some(last_live) = live_ranges.iter().rposition(|r| r.end != 0) {
            live_ranges.truncate(last_live + 1);
        }

        RegAllocations::run(self.func, live_ranges);
    }

    fn emit_call(&mut self, idx: u32) {
        let inst = Instruction {
            kind: InstructionKind::Call { idx },
            ..Instruction::default()
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_nop(&mut self) {}

    fn emit_int_add(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction {
            kind: InstructionKind::IntAdd,
            dst: [self.def_var(dst)],
            src: [self.use_var(a), self.use_var(b), Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_sub(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction {
            kind: InstructionKind::IntSub,
            dst: [self.def_var(dst)],
            src: [self.use_var(a), self.use_var(b), Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_mul(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction {
            kind: InstructionKind::IntMul,
            dst: [self.def_var(dst)],
            src: [self.use_var(a), self.use_var(b), Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_mul_high(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction {
            kind: InstructionKind::IntMulHigh,
            dst: [self.def_var(dst)],
            src: [self.use_var(a), self.use_var(b), Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_mul_high_unsigned(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction {
            kind: InstructionKind::IntMulHighUnsigned,
            dst: [self.def_var(dst)],
            src: [self.use_var(a), self.use_var(b), Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_neg(&mut self, dst: u8, src: u8) {
        let inst = Instruction {
            kind: InstructionKind::IntNeg,
            dst: [self.def_var(dst)],
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_abs(&mut self, dst: u8, src: u8) {
        let inst = Instruction {
            kind: InstructionKind::IntAbs,
            dst: [self.def_var(dst)],
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_inc(&mut self, dst: u8) {
        let inst = Instruction {
            kind: InstructionKind::IntInc,
            dst: [self.def_var(dst)],
            src: [self.use_var(dst), Var::INVALID, Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_dec(&mut self, dst: u8) {
        let inst = Instruction {
            kind: InstructionKind::IntDec,
            dst: [self.def_var(dst)],
            src: [self.use_var(dst), Var::INVALID, Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_min(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction {
            kind: InstructionKind::IntMin,
            dst: [self.def_var(dst)],
            src: [self.use_var(a), self.use_var(b), Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_max(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction {
            kind: InstructionKind::IntMax,
            dst: [self.def_var(dst)],
            src: [self.use_var(a), self.use_var(b), Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_or(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction {
            kind: InstructionKind::BitOr,
            dst: [self.def_var(dst)],
            src: [self.use_var(a), self.use_var(b), Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_and(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction {
            kind: InstructionKind::BitAnd,
            dst: [self.def_var(dst)],
            src: [self.use_var(a), self.use_var(b), Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_xor(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction {
            kind: InstructionKind::BitXor,
            dst: [self.def_var(dst)],
            src: [self.use_var(a), self.use_var(b), Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_not(&mut self, dst: u8, src: u8) {
        let inst = Instruction {
            kind: InstructionKind::BitNot,
            dst: [self.def_var(dst)],
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_shift_left(&mut self, dst: u8, src: u8, amount: u8) {
        let inst = Instruction {
            kind: InstructionKind::BitShiftLeft { amount },
            dst: [self.def_var(dst)],
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_shift_right(&mut self, dst: u8, src: u8, amount: u8) {
        let inst = Instruction {
            kind: InstructionKind::BitShiftRight { amount },
            dst: [self.def_var(dst)],
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_rotate_left(&mut self, dst: u8, src: u8, amount: u8) {
        let inst = Instruction {
            kind: InstructionKind::BitRotateLeft { amount },
            dst: [self.def_var(dst)],
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_rotate_right(&mut self, dst: u8, src: u8, amount: u8) {
        let inst = Instruction {
            kind: InstructionKind::BitRotateRight { amount },
            dst: [self.def_var(dst)],
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_select(&mut self, dst: u8, mask: u8, a: u8, b: u8) {
        let inst = Instruction {
            kind: InstructionKind::BitSelect,
            dst: [self.def_var(dst)],
            src: [self.use_var(mask), self.use_var(a), self.use_var(b)],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_popcnt(&mut self, dst: u8, src: u8) {
        let inst = Instruction {
            kind: InstructionKind::BitPopcnt,
            dst: [self.def_var(dst)],
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_reverse(&mut self, dst: u8, src: u8) {
        let inst = Instruction {
            kind: InstructionKind::BitReverse,
            dst: [self.def_var(dst)],
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_branch_cmp(&mut self, a: u8, b: u8, compare_kind: CompareKind, offset: u32) {
        let inst = Instruction {
            kind: InstructionKind::BranchCmp { compare_kind },
            src: [self.use_var(a), self.use_var(b), Var::INVALID],
            ..Instruction::default()
        };
        self.finish_block_with_branch(inst, offset);
    }

    fn emit_branch_zero(&mut self, src: u8, offset: u32) {
        let inst = Instruction {
            kind: InstructionKind::BranchZero,
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
            ..Instruction::default()
        };
        self.finish_block_with_branch(inst, offset);
    }

    fn emit_branch_non_zero(&mut self, src: u8, offset: u32) {
        let inst = Instruction {
            kind: InstructionKind::BranchNonZero,
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
            ..Instruction::default()
        };
        self.finish_block_with_branch(inst, offset);
    }

    fn emit_mem_load(&mut self, dst: u8, addr: u32) {
        let inst = Instruction {
            kind: InstructionKind::MemLoad { addr },
            dst: [self.def_var(dst)],
            ..Instruction::default()
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_mem_store(&mut self, addr: u32, src: u8) {
        let inst = Instruction {
            kind: InstructionKind::MemStore { addr },
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
            ..Instruction::default()
        };
        self.cur_block.instructions.push(inst);
    }
}

#[derive(Debug, Default)]
pub struct Function {
    pub blocks: Vec<Block>,
    pub reg_allocs: RegAllocations,
}

#[derive(Debug)]
pub struct Block {
    predecessors: Vec<BlockName>,
    params: Vec<Var>,
    var_def_mask: VarMask,
    pub instructions: Vec<Instruction>,
    pub exit: BlockName,
    pub branch_exit: BlockName,
}

impl Default for Block {
    fn default() -> Self {
        Self {
            predecessors: vec![],
            params: vec![],
            var_def_mask: VarMask::EMPTY,
            instructions: vec![],
            exit: BlockName::INVALID,
            branch_exit: BlockName::INVALID,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Var(u32);

impl Var {
    const INVALID: Self = Self(u32::MAX);

    fn new(name: u8) -> Self {
        Self((name as u32) << 26)
    }

    fn is_valid(self) -> bool {
        self != Self::INVALID
    }

    #[inline]
    fn name(self) -> u8 {
        (self.0 >> 26) as u8
    }

    #[inline]
    fn version(self) -> u32 {
        self.0 & 0x03FFFFFF
    }

    #[inline]
    fn set_version(&mut self, version: u32) {
        self.0 &= 0xFC000000;
        self.0 |= 0x03FFFFFF & version;
    }
}

impl Debug for Var {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if *self == Self::INVALID {
            f.write_str("INVALID")
        } else {
            f.write_fmt(format_args!("v{}_{}", self.name(), self.version()))
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct VarMask(u64);

impl VarMask {
    const ALL: Self = Self(u64::MAX);
    const EMPTY: Self = Self(0);

    #[inline]
    fn insert(&mut self, var_name: u8) {
        self.0 |= 1 << var_name;
    }

    #[inline]
    fn contains(self, var_name: u8) -> bool {
        self.0 & (1 << var_name) != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LiveRange {
    pub var: Var,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockName(pub u32);

impl BlockName {
    pub const INVALID: Self = Self(u32::MAX);

    pub fn is_valid(self) -> bool {
        self != Self::INVALID
    }
}

pub struct PendingBranchTarget {
    branch_proxy_block_name: BlockName,
    target_instruction: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Instruction {
    pub kind: InstructionKind,
    dst: [Var; 1],
    src: [Var; 3],
}

impl Instruction {
    fn jump() -> Self {
        Self {
            kind: InstructionKind::Jump,
            ..Self::default()
        }
    }

    fn return_() -> Self {
        Self {
            kind: InstructionKind::Return,
            ..Self::default()
        }
    }

    pub fn dst_iter(&self) -> impl Iterator<Item = Var> {
        self.dst.into_iter().take_while(|v| v.is_valid())
    }

    fn dst_iter_mut(&mut self) -> impl Iterator<Item = &mut Var> {
        self.dst.iter_mut().take_while(|v| v.is_valid())
    }

    pub fn src_iter(&self) -> impl Iterator<Item = Var> {
        self.src.into_iter().take_while(|v| v.is_valid())
    }

    fn src_iter_mut(&mut self) -> impl Iterator<Item = &mut Var> {
        self.src.iter_mut().take_while(|v| v.is_valid())
    }
}

impl Default for Instruction {
    fn default() -> Self {
        Self {
            kind: InstructionKind::Return,
            dst: [Var::INVALID; 1],
            src: [Var::INVALID; 3],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InstructionKind {
    Return,
    Jump,
    InitVar,

    Call { idx: u32 },
    BranchCmp { compare_kind: CompareKind },
    BranchZero,
    BranchNonZero,
    IntAdd,
    IntSub,
    IntMul,
    IntMulHigh,
    IntMulHighUnsigned,
    IntNeg,
    IntAbs,
    IntInc,
    IntDec,
    IntMin,
    IntMax,
    BitOr,
    BitAnd,
    BitXor,
    BitNot,
    BitShiftLeft { amount: u8 },
    BitShiftRight { amount: u8 },
    BitRotateLeft { amount: u8 },
    BitRotateRight { amount: u8 },
    BitSelect,
    BitPopcnt,
    BitReverse,
    MemLoad { addr: u32 },
    MemStore { addr: u32 },
}
