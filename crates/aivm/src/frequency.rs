pub trait InstructionFrequencies {
    const END_FUNC: u16 = 3449;
    const CALL: u16 = 3449;

    const INT_ADD: u16 = 3449;
    const INT_SUB: u16 = 3449;
    const INT_MUL: u16 = 3453;
    const INT_MUL_HIGH: u16 = 3449;
    const INT_MUL_HIGH_UNSIGNED: u16 = 3449;
    const INT_NEG: u16 = 3449;

    const BIT_SWAP: u16 = 3449;
    const BIT_OR: u16 = 3449;
    const BIT_AND: u16 = 3449;
    const BIT_XOR: u16 = 3449;
    const BIT_SHIFT_L: u16 = 3449;
    const BIT_SHIFT_R: u16 = 3449;
    const BIT_ROT_L: u16 = 3449;
    const BIT_ROT_R: u16 = 3449;

    const COND_BRANCH: u16 = 3449;

    const MEM_LOAD: u16 = 3449;
    const MEM_STORE: u16 = 3449;
}

pub struct DefaultFrequencies(());

impl InstructionFrequencies for DefaultFrequencies {}
