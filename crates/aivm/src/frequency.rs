pub trait InstructionFrequencies {
    const END_FUNC: u16 = 2184;
    const CALL: u16 = 2184;

    const INT_ADD: u16 = 2184;
    const INT_SUB: u16 = 2184;
    const INT_MUL: u16 = 2199;
    const INT_MUL_HIGH: u16 = 2184;
    const INT_MUL_HIGH_UNSIGNED: u16 = 2184;
    const INT_NEG: u16 = 2184;
    const INT_ABS: u16 = 2184;
    const INT_INC: u16 = 2184;
    const INT_DEC: u16 = 2184;
    const INT_MIN: u16 = 2184;
    const INT_MAX: u16 = 2184;

    const BIT_SWAP: u16 = 2184;
    const BIT_OR: u16 = 2184;
    const BIT_AND: u16 = 2184;
    const BIT_XOR: u16 = 2184;
    const BIT_NOT: u16 = 2184;
    const BIT_SHIFT_L: u16 = 2184;
    const BIT_SHIFT_R: u16 = 2184;
    const BIT_ROT_L: u16 = 2184;
    const BIT_ROT_R: u16 = 2184;
    const BIT_SELECT: u16 = 2184;
    const BIT_POPCNT: u16 = 2184;
    const BIT_REVERSE: u16 = 2184;

    const BRANCH_CMP: u16 = 2184;
    const BRANCH_ZERO: u16 = 2184;
    const BRANCH_NON_ZERO: u16 = 2184;

    const MEM_LOAD: u16 = 2184;
    const MEM_STORE: u16 = 2184;
}

pub struct DefaultFrequencies(());

impl InstructionFrequencies for DefaultFrequencies {}
