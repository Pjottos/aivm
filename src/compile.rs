use crate::{
    codegen::{self, CodeGenerator},
    Runner,
};

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum CodeGenKind {
    Fastest,
    Interpreter,
}

impl Default for CodeGenKind {
    fn default() -> Self {
        Self::Fastest
    }
}

impl CodeGenKind {
    pub fn is_supported(self) -> bool {
        match self {
            Self::Fastest | Self::Interpreter => true,
        }
    }
}

pub fn compile_program(
    code: &[f32],
    mem_size: usize,
    mut code_gen_kind: CodeGenKind,
) -> Box<dyn Runner> {
    if let CodeGenKind::Fastest = code_gen_kind {
        code_gen_kind = CodeGenKind::Interpreter;
    }

    match code_gen_kind {
        CodeGenKind::Interpreter => Box::new(compile::<codegen::Interpreter>(code, mem_size)),
        CodeGenKind::Fastest => unreachable!(),
    }
}

fn compile<G: CodeGenerator>(code: &[f32], mem_size: usize) -> G::Runner {
    todo!()
}
