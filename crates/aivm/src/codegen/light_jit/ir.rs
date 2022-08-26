use crate::{
    codegen,
    compile::{CompareKind, MemoryBank},
};

pub struct Emitter<'a> {
    func: &'a mut Function,
    var_versions: [u32; 64],
    instruction_count: u32,
    block_targets: Vec<(BlockName, u32)>,
    cur_block: Block,
}

impl<'a> Emitter<'a> {
    pub fn new(func: &'a mut Function) -> Self {
        Self {
            func,
            var_versions: [0; 64],
            instruction_count: 0,
            block_targets: vec![],
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
        std::mem::swap(&mut self.cur_block, &mut block);
        self.func.blocks.push(block);
    }

    fn finish_block_with_branch(&mut self, inst: Instruction, offset: u32) {
        let next_block_name = self.next_block_name();
        let cur_block_name = self.cur_block_name();

        let block = &mut self.cur_block;
        block.terminator_idx = block.instructions.len();
        block.instructions.push(inst);
        block.exit.target = next_block_name;

        let target_instruction = self.instruction_count - 1 + offset;
        self.block_targets
            .push((cur_block_name, target_instruction));

        self.finish_block();
    }

    fn finish_block_with_fall_through(&mut self) {
        self.cur_block.terminator_idx = self.cur_block.instructions.len();
        self.cur_block.instructions.push(Instruction::FallThrough);
        self.cur_block.exit.target = self.next_block_name();
        self.finish_block();
    }

    fn def_var(&mut self, name: u8) -> Var {
        self.var_versions[name as usize] += 1;
        self.use_var(name)
    }

    fn use_var(&self, name: u8) -> Var {
        Var {
            name,
            version: self.var_versions[name as usize],
        }
    }
}

impl<'a> codegen::private::Emitter for Emitter<'a> {
    fn prepare_emit(&mut self) {
        // Use `drain_filter` when stabilized (https://github.com/rust-lang/rust/issues/43244)
        let mut i = 0;
        while i < self.block_targets.len() {
            if self.block_targets[i].1 == self.instruction_count {
                let block_idx = self.block_targets[i].0 .0 as usize;

                self.block_targets.swap_remove(i);
                // Begin new block for branch to jump to
                if !self.cur_block.instructions.is_empty() {
                    self.finish_block_with_fall_through();
                }

                self.func.blocks[block_idx].branch_exit = Some(BlockExit {
                    target: self.next_block_name(),
                    args: vec![],
                });
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
    instructions: Vec<Instruction>,
    terminator_idx: usize,
    exit: BlockExit,
    branch_exit: Option<BlockExit>,
}

impl Default for Block {
    fn default() -> Self {
        Self {
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
