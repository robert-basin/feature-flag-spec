//! Canonical pretty printer for parsed flags.
//!
//! Each `Flag` is written independently through `write_flag`, so a caller
//! streaming flags out of `FlagReader` can print them one at a time without
//! ever assembling the full output in memory either.

use crate::{Flag, Value};
use std::io::{self, Write};

pub fn write_flag<W: Write>(w: &mut W, flag: &Flag) -> io::Result<()> {
    writeln!(w, "flag {} {{", flag.name)?;
    if let Some(desc) = &flag.description {
        writeln!(w, "    description = {}", quote(desc))?;
    }
    writeln!(w, "    enabled = {}", flag.enabled)?;
    if let Some(r) = flag.rollout {
        writeln!(w, "    rollout = {}", r)?;
    }
    for rule in &flag.rules {
        writeln!(
            w,
            "    rule {} {} {} => {}",
            rule.field,
            rule.op.as_str(),
            format_value(&rule.value),
            rule.result
        )?;
    }
    if let Some(d) = flag.default {
        writeln!(w, "    default = {}", d)?;
    }
    writeln!(w, "}}")
}

fn format_value(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Str(s) => quote(s),
    }
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}
