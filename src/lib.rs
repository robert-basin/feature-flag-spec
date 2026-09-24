//! Types and modules for the flagspec feature flag format.
//!
//! A flagspec file is a sequence of `flag <name> { ... }` blocks. See
//! README.md for the full grammar and examples.

pub mod parser;
pub mod printer;

use std::fmt;

/// A scalar value used in rule conditions and results.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Str(String),
}

/// Comparison operator used in a rule condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
}

impl Op {
    pub fn as_str(&self) -> &'static str {
        match self {
            Op::Eq => "==",
            Op::Ne => "!=",
        }
    }
}

/// One `rule <field> <op> <value> => <bool>` line inside a flag block.
#[derive(Debug, Clone)]
pub struct Rule {
    pub field: String,
    pub op: Op,
    pub value: Value,
    pub result: bool,
}

/// A single parsed and validated `flag` block.
#[derive(Debug, Clone)]
pub struct Flag {
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub rollout: Option<u8>,
    pub rules: Vec<Rule>,
    pub default: Option<bool>,
}

/// A validation or syntax error tied to a specific line in the source.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}
