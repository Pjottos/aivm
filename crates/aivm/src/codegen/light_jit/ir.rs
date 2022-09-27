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
                    .map(|i| Instruction::Const {
                        dst: Var {
                            name: i,
                            version: 0,
                        },
                        val: 0,
                    })
                    .collect(),
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
        if self.cur_block.exit.target == self.next_block_name() {
            block.predecessors.push(self.cur_block_name());
        }

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
        block.exit.target = fall_through_proxy_block_name;
        block.branch_exit = Some(BlockExit {
            target: branch_proxy_block_name,
            args: vec![],
        });
        self.finish_block();

        // Split critical edges, since unconditional jumps don't exist and therefore all blocks
        // have at least 1 predecessor (except the first block but it can never be the target
        // of a branch since branches don't go backward) all edges where a branch is taken are
        // critical. Edges where the branch is not taken are potentially critical if the following
        // block is the target of another branch instruction so we generate a proxy block for
        // that first
        let fall_through_proxy_block = Block {
            predecessors: vec![block_name],
            terminator_idx: 0,
            instructions: vec![Instruction::FallThrough],
            exit: BlockExit {
                target: next_block_name,
                args: vec![],
            },
            branch_exit: None,
        };
        self.func.blocks.push(fall_through_proxy_block);

        let branch_proxy_block = Block {
            predecessors: vec![block_name],
            terminator_idx: 0,
            instructions: vec![Instruction::FallThrough],
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
        self.cur_block.terminator_idx = self.cur_block.instructions.len();
        self.cur_block.instructions.push(Instruction::FallThrough);
        self.cur_block.exit.target = self.next_block_name();
        self.finish_block();
    }

    fn def_var(&mut self, name: u8) -> Var {
        self.use_var(name)
    }

    fn use_var(&self, name: u8) -> Var {
        Var { name, version: 0 }
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
                self.func.blocks[branch_proxy_block_name.0 as usize]
                    .exit
                    .target = self.cur_block_name();
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
        self.cur_block.instructions.push(Instruction::Return);
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
    }

    fn emit_call(&mut self, idx: u32) {
        let next_block_name = self.next_block_name();
        let block = &mut self.cur_block;
        block.terminator_idx = block.instructions.len();
        block.instructions.push(Instruction::Call { idx });
        block.exit.target = next_block_name;
        self.finish_block();
    }

    fn emit_nop(&mut self) {}

    fn emit_int_add(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction::IntAdd {
            dst: self.def_var(dst),
            a: self.use_var(a),
            b: self.use_var(b),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_sub(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction::IntSub {
            dst: self.def_var(dst),
            a: self.use_var(a),
            b: self.use_var(b),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_mul(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction::IntMul {
            dst: self.def_var(dst),
            a: self.use_var(a),
            b: self.use_var(b),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_mul_high(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction::IntMulHigh {
            dst: self.def_var(dst),
            a: self.use_var(a),
            b: self.use_var(b),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_mul_high_unsigned(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction::IntMulHighUnsigned {
            dst: self.def_var(dst),
            a: self.use_var(a),
            b: self.use_var(b),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_neg(&mut self, dst: u8, src: u8) {
        let inst = Instruction::IntNeg {
            dst: self.def_var(dst),
            src: self.use_var(src),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_abs(&mut self, dst: u8, src: u8) {
        let inst = Instruction::IntAbs {
            dst: self.def_var(dst),
            src: self.use_var(src),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_inc(&mut self, dst: u8) {
        let inst = Instruction::IntInc {
            src: self.use_var(dst),
            dst: self.def_var(dst),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_dec(&mut self, dst: u8) {
        let inst = Instruction::IntDec {
            src: self.use_var(dst),
            dst: self.def_var(dst),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_min(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction::IntMin {
            dst: self.def_var(dst),
            a: self.use_var(a),
            b: self.use_var(b),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_int_max(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction::IntMax {
            dst: self.def_var(dst),
            a: self.use_var(a),
            b: self.use_var(b),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_swap(&mut self, dst: u8, src: u8) {
        let inst = Instruction::BitSwap {
            dst: self.def_var(dst),
            src: self.def_var(src),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_or(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction::BitOr {
            dst: self.def_var(dst),
            a: self.use_var(a),
            b: self.use_var(b),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_and(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction::BitAnd {
            dst: self.def_var(dst),
            a: self.use_var(a),
            b: self.use_var(b),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_xor(&mut self, dst: u8, a: u8, b: u8) {
        let inst = Instruction::BitXor {
            dst: self.def_var(dst),
            a: self.use_var(a),
            b: self.use_var(b),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_not(&mut self, dst: u8, src: u8) {
        let inst = Instruction::BitNot {
            dst: self.def_var(dst),
            src: self.use_var(src),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_shift_left(&mut self, dst: u8, src: u8, amount: u8) {
        let inst = Instruction::BitShiftLeft {
            dst: self.def_var(dst),
            src: self.use_var(src),
            amount,
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_shift_right(&mut self, dst: u8, src: u8, amount: u8) {
        let inst = Instruction::BitShiftRight {
            dst: self.def_var(dst),
            src: self.use_var(src),
            amount,
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_rotate_left(&mut self, dst: u8, src: u8, amount: u8) {
        let inst = Instruction::BitRotateLeft {
            dst: self.def_var(dst),
            src: self.use_var(src),
            amount,
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_rotate_right(&mut self, dst: u8, src: u8, amount: u8) {
        let inst = Instruction::BitRotateRight {
            dst: self.def_var(dst),
            src: self.use_var(src),
            amount,
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_select(&mut self, dst: u8, mask: u8, a: u8, b: u8) {
        let inst = Instruction::BitSelect {
            dst: self.def_var(dst),
            mask: self.use_var(mask),
            a: self.use_var(a),
            b: self.use_var(b),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_popcnt(&mut self, dst: u8, src: u8) {
        let inst = Instruction::BitPopcnt {
            dst: self.def_var(dst),
            src: self.use_var(src),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_bit_reverse(&mut self, dst: u8, src: u8) {
        let inst = Instruction::BitReverse {
            dst: self.def_var(dst),
            src: self.use_var(src),
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_branch_cmp(&mut self, a: u8, b: u8, compare_kind: CompareKind, offset: u32) {
        let inst = Instruction::BranchCmp {
            a: self.use_var(a),
            b: self.use_var(b),
            compare_kind,
        };
        self.finish_block_with_branch(inst, offset);
    }

    fn emit_branch_zero(&mut self, src: u8, offset: u32) {
        let inst = Instruction::BranchZero {
            src: self.use_var(src),
        };
        self.finish_block_with_branch(inst, offset);
    }

    fn emit_branch_non_zero(&mut self, src: u8, offset: u32) {
        let inst = Instruction::BranchNonZero {
            src: self.use_var(src),
        };
        self.finish_block_with_branch(inst, offset);
    }

    fn emit_mem_load(&mut self, bank: MemoryBank, dst: u8, addr: u32) {
        let inst = Instruction::MemLoad {
            bank,
            dst: self.def_var(dst),
            addr,
        };
        self.cur_block.instructions.push(inst);
    }

    fn emit_mem_store(&mut self, bank: MemoryBank, addr: u32, src: u8) {
        let inst = Instruction::MemStore {
            bank,
            addr,
            src: self.use_var(src),
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
    terminator_idx: usize,
    exit: BlockExit,
    branch_exit: Option<BlockExit>,
}

impl Default for Block {
    fn default() -> Self {
        Self {
            predecessors: vec![],
            instructions: vec![],
            terminator_idx: usize::MAX,
            exit: BlockExit {
                target: BlockName::INVALID,
                args: vec![],
            },
            branch_exit: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Var {
    name: u8,
    version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockName(u32);

impl BlockName {
    pub const INVALID: Self = Self(u32::MAX);
}

#[derive(Debug)]
pub struct BlockExit {
    target: BlockName,
    args: Vec<Var>,
}

pub struct PendingBranchTarget {
    branch_proxy_block_name: BlockName,
    target_instruction: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum Instruction {
    Return,
    FallThrough,
    Move {
        dst: Var,
        src: Var,
    },
    Const {
        dst: Var,
        val: i64,
    },

    Call {
        idx: u32,
    },
    BranchCmp {
        a: Var,
        b: Var,
        compare_kind: CompareKind,
    },
    BranchZero {
        src: Var,
    },
    BranchNonZero {
        src: Var,
    },
    IntAdd {
        dst: Var,
        a: Var,
        b: Var,
    },
    IntSub {
        dst: Var,
        a: Var,
        b: Var,
    },
    IntMul {
        dst: Var,
        a: Var,
        b: Var,
    },
    IntMulHigh {
        dst: Var,
        a: Var,
        b: Var,
    },
    IntMulHighUnsigned {
        dst: Var,
        a: Var,
        b: Var,
    },
    IntNeg {
        dst: Var,
        src: Var,
    },
    IntAbs {
        dst: Var,
        src: Var,
    },
    IntInc {
        dst: Var,
        src: Var,
    },
    IntDec {
        dst: Var,
        src: Var,
    },
    IntMin {
        dst: Var,
        a: Var,
        b: Var,
    },
    IntMax {
        dst: Var,
        a: Var,
        b: Var,
    },
    BitSwap {
        dst: Var,
        src: Var,
    },
    BitOr {
        dst: Var,
        a: Var,
        b: Var,
    },
    BitAnd {
        dst: Var,
        a: Var,
        b: Var,
    },
    BitXor {
        dst: Var,
        a: Var,
        b: Var,
    },
    BitNot {
        dst: Var,
        src: Var,
    },
    BitShiftLeft {
        dst: Var,
        src: Var,
        amount: u8,
    },
    BitShiftRight {
        dst: Var,
        src: Var,
        amount: u8,
    },
    BitRotateLeft {
        dst: Var,
        src: Var,
        amount: u8,
    },
    BitRotateRight {
        dst: Var,
        src: Var,
        amount: u8,
    },
    BitSelect {
        dst: Var,
        mask: Var,
        a: Var,
        b: Var,
    },
    BitPopcnt {
        dst: Var,
        src: Var,
    },
    BitReverse {
        dst: Var,
        src: Var,
    },
    MemLoad {
        bank: MemoryBank,
        dst: Var,
        addr: u32,
    },
    MemStore {
        bank: MemoryBank,
        addr: u32,
        src: Var,
    },
}