use crate::parser::ir::RegBank;

#[derive(Debug, Clone)]
pub enum ParseError {
    /// Lexer couldn't classify a character sequence.
    UnknownToken { line: usize, text: String },

    /// A chumsky grammar rule failed to match.
    UnexpectedToken {
        line: usize,
        expected: String,
        found: String,
    },

    /// The (mnemonic, modifiers) combination doesn't correspond to any
    /// `InstType` variant.
    UnknownOpcode {
        line: usize,
        mnemonic: String,
        modifiers: Vec<String>,
    },

    /// An instruction had operand types that don't match any supported
    /// `InstType` variant.
    UnsupportedOperandShape {
        line: usize,
        opcode: String,
        reason: String,
    },

    /// Branch target doesn't appear anywhere in the kernel.
    UndefinedLabel { line: usize, label: String },

    /// `ld.param` referenced a name not in the kernel's parameter list.
    UndefinedParam { line: usize, param: String },

    /// Kernel body contained no `.entry` block.
    MissingEntry,

    /// Kernel declared a register outside the bounds of Zekai's 256-entry
    /// register files.
    RegisterOutOfRange {
        line: usize,
        bank: RegBank,
        index: u32,
    },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownToken { line, text } => {
                write!(f, "line {line}: unknown token `{text}`")
            }
            Self::UnexpectedToken { line, expected, found } => {
                write!(f, "line {line}: expected {expected}, found `{found}`")
            }
            Self::UnknownOpcode {
                line,
                mnemonic,
                modifiers,
            } => {
                let mods: String = modifiers.iter().map(|m| format!(".{m}")).collect();
                write!(f, "line {line}: unknown opcode `{mnemonic}{mods}`")
            }
            Self::UnsupportedOperandShape {
                line,
                opcode,
                reason,
            } => {
                write!(
                    f,
                    "line {line}: `{opcode}` has unsupported operand shape: {reason}"
                )
            }
            Self::UndefinedLabel { line, label } => {
                write!(f, "line {line}: undefined label `{label}`")
            }
            Self::UndefinedParam { line, param } => {
                write!(f, "line {line}: undefined parameter `{param}`")
            }
            Self::MissingEntry => write!(f, "PTX contained no .entry block"),
            Self::RegisterOutOfRange { line, bank, index } => {
                write!(
                    f,
                    "line {line}: register %{bank:?}{index} out of range (max 255)"
                )
            }
        }
    }
}

impl std::error::Error for ParseError {}