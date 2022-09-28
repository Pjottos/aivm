use std::fmt::Debug;

use bitvec::prelude::*;

use crate::{
    codegen,
    compile::{CompareKind, MemoryBank},
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
                        kind: InstructionKind::Const { val: 0 },
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

        let block = &mut self.cur_block;
        block.terminator_idx = block.instructions.len();
        block.instructions.push(inst);
        block.exit = fall_through_proxy_block_name;
        block.branch_exit = branch_proxy_block_name;
        self.finish_block();
        self.cur_block
            .predecessors
            .push(fall_through_proxy_block_name);

        // Split critical edges, since unconditional jumps don't exist and therefore all blocks
        // have at least 1 predecessor (except the first block but it can never be the target
        // of a branch since branches don't go backward) all edges where a branch is taken are
        // critical. Edges where the branch is not taken are potentially critical if the following
        // block is the target of another branch instruction so we generate a proxy block for
        // that first
        let fall_through_proxy_block = Block {
            predecessors: vec![block_name],
            terminator_idx: 0,
            instructions: vec![Instruction::fall_through()],
            exit: next_block_name,
            ..Block::default()
        };
        self.func.blocks.push(fall_through_proxy_block);

        let branch_proxy_block = Block {
            predecessors: vec![block_name],
            terminator_idx: 0,
            instructions: vec![Instruction::fall_through()],
            ..Block::default()
        };
        self.func.blocks.push(branch_proxy_block);

        let target_instruction = self.instruction_count - 1 + offset;
        self.branch_targets.push(PendingBranchTarget {
            branch_proxy_block_name,
            target_instruction,
        });
    }

    fn finish_block_with_fall_through(&mut self) {
        let block_name = self.cur_block_name();
        self.cur_block.terminator_idx = self.cur_block.instructions.len();
        self.cur_block
            .instructions
            .push(Instruction::fall_through());
        self.cur_block.exit = self.next_block_name();
        self.finish_block();
        self.cur_block.predecessors.push(block_name);
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

        self.instruction_count += 1;
    }

    fn finalize(&mut self) {
        if !self.cur_block.instructions.is_empty() {
            self.finish_block_with_fall_through();
        }

        self.cur_block.terminator_idx = 0;
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
                    .find(|p| doms[p.0 as usize] != BlockName::INVALID)
                    .unwrap();
                let initial_idom = new_idom;

                for predecessor in block
                    .predecessors
                    .iter()
                    .copied()
                    .filter(|&p| p != initial_idom)
                {
                    if doms[predecessor.0 as usize] != BlockName::INVALID {
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

        let mut gen_name = |v: &mut Var, var_stacks: &mut [Vec<u32>]| {
            let counter = &mut version_counters[v.name() as usize];
            v.set_version(*counter);
            var_stacks[v.name() as usize].push(*counter);
            *counter += 1;
        };

        block_stack.push((BlockName(0), BlockName(0)));
        while let Some((b, last_child)) = block_stack.pop() {
            let block = &mut self.func.blocks[b.0 as usize];
            if b == last_child {
                for var in &mut block.params {
                    gen_name(var, &mut var_stacks);
                }
                for inst in &mut block.instructions {}
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

            for &var in &block.params {
                var_stacks[var.name() as usize].pop();
            }
            for inst in &mut block.instructions {}
        }

        println!("func: {:#?}", self.func.blocks);
        //for (i, dom) in doms.iter().enumerate() {
        //    println!("{}: {}", i, dom.0);
        //}
        panic!();
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

    fn emit_bit_swap(&mut self, dst: u8, src: u8) {
        let inst = Instruction {
            kind: InstructionKind::BitSwap,
            dst: [self.def_var(dst)],
            src: [self.def_var(src), Var::INVALID, Var::INVALID],
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

    fn emit_mem_load(&mut self, bank: MemoryBank, dst: u8, addr: u32) {
        let inst = Instruction {
            kind: InstructionKind::MemLoad { bank, addr },
            dst: [self.def_var(dst)],
            ..Instruction::default()
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_mem_store(&mut self, bank: MemoryBank, addr: u32, src: u8) {
        let inst = Instruction {
            kind: InstructionKind::MemStore { bank, addr },
            src: [self.use_var(src), Var::INVALID, Var::INVALID],
            ..Instruction::default()
        };
        self.cur_block.instructions.push(inst);
    }
}

#[derive(Debug, Default)]
pub struct Function {
    pub blocks: Vec<Block>,
}

#[derive(Debug)]
pub struct Block {
    predecessors: Vec<BlockName>,
    instructions: Vec<Instruction>,
    params: Vec<Var>,
    var_def_mask: VarMask,
    terminator_idx: usize,
    exit: BlockName,
    branch_exit: BlockName,
}

impl Default for Block {
    fn default() -> Self {
        Self {
            predecessors: vec![],
            instructions: vec![],
            params: vec![],
            var_def_mask: VarMask::EMPTY,
            terminator_idx: usize::MAX,
            exit: BlockName::INVALID,
            branch_exit: BlockName::INVALID,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Var(u32);

impl Var {
    const INVALID: Self = Self(u32::MAX);

    fn new(name: u8) -> Self {
        Self((name as u32) << 26)
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
            f.write_str("Var::INVALID")
        } else {
            f.debug_struct("Var")
                .field("name", &self.name())
                .field("version", &self.version())
                .finish()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockName(u32);

impl BlockName {
    pub const INVALID: Self = Self(u32::MAX);
}

pub struct PendingBranchTarget {
    branch_proxy_block_name: BlockName,
    target_instruction: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Instruction {
    kind: InstructionKind,
    dst: [Var; 1],
    src: [Var; 3],
}

impl Instruction {
    fn fall_through() -> Self {
        Self {
            kind: InstructionKind::FallThrough,
            ..Self::default()
        }
    }

    fn return_() -> Self {
        Self {
            kind: InstructionKind::Return,
            ..Self::default()
        }
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
    FallThrough,
    Const { val: i64 },

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
    BitSwap,
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
    MemLoad { bank: MemoryBank, addr: u32 },
    MemStore { bank: MemoryBank, addr: u32 },
}
