//! Streaming parser for flagspec files.
//!
//! `FlagReader` wraps any `BufRead` and yields one validated `Flag` at a
//! time. Only the lines belonging to the flag block currently being read
//! are held in memory; once a block is parsed its buffer is dropped before
//! the next one starts, so a file with a million flags costs about as much
//! memory as its single largest block, not the whole file.

use crate::{Flag, Op, ParseError, Rule, Value};
use std::fmt;
use std::io::BufRead;

/// Either an I/O failure or a validation/syntax error at a known line.
#[derive(Debug)]
pub enum FlagError {
    Io(std::io::Error),
    Parse(ParseError),
}

impl fmt::Display for FlagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlagError::Io(e) => write!(f, "io error: {}", e),
            FlagError::Parse(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for FlagError {}

pub struct FlagReader<R> {
    inner: R,
    line_no: usize,
    done: bool,
}

impl<R: BufRead> FlagReader<R> {
    pub fn new(inner: R) -> Self {
        FlagReader {
            inner,
            line_no: 0,
            done: false,
        }
    }

    fn read_line(&mut self) -> std::io::Result<Option<String>> {
        let mut buf = String::new();
        let n = self.inner.read_line(&mut buf)?;
        if n == 0 {
            return Ok(None);
        }
        self.line_no += 1;
        while buf.ends_with('\n') || buf.ends_with('\r') {
            buf.pop();
        }
        Ok(Some(buf))
    }
}

impl<R: BufRead> Iterator for FlagReader<R> {
    type Item = Result<Flag, FlagError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        loop {
            let line = match self.read_line() {
                Ok(Some(l)) => l,
                Ok(None) => {
                    self.done = true;
                    return None;
                }
                Err(e) => {
                    self.done = true;
                    return Some(Err(FlagError::Io(e)));
                }
            };
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let header_line = self.line_no;
            let rest = match trimmed.strip_prefix("flag ") {
                Some(r) => r,
                None => {
                    self.done = true;
                    return Some(Err(FlagError::Parse(ParseError {
                        line: header_line,
                        message: format!("expected 'flag <name> {{', found '{}'", trimmed),
                    })));
                }
            };
            let name = match rest.trim_end().strip_suffix('{') {
                Some(n) => n.trim().to_string(),
                None => {
                    self.done = true;
                    return Some(Err(FlagError::Parse(ParseError {
                        line: header_line,
                        message: "expected '{' at the end of the flag header".to_string(),
                    })));
                }
            };
            if let Err(e) = validate_name(&name, header_line) {
                self.done = true;
                return Some(Err(FlagError::Parse(e)));
            }

            let mut body: Vec<(usize, String)> = Vec::new();
            loop {
                let body_line = match self.read_line() {
                    Ok(Some(l)) => l,
                    Ok(None) => {
                        self.done = true;
                        return Some(Err(FlagError::Parse(ParseError {
                            line: self.line_no,
                            message: format!(
                                "unterminated flag block opened at line {}",
                                header_line
                            ),
                        })));
                    }
                    Err(e) => {
                        self.done = true;
                        return Some(Err(FlagError::Io(e)));
                    }
                };
                if body_line.trim() == "}" {
                    break;
                }
                let line_no = self.line_no;
                body.push((line_no, body_line));
            }

            return Some(parse_flag_body(name, header_line, body).map_err(FlagError::Parse));
        }
    }
}

fn validate_name(name: &str, line: usize) -> Result<(), ParseError> {
    if name.is_empty() {
        return Err(ParseError {
            line,
            message: "flag name must not be empty".to_string(),
        });
    }
    let first = name.chars().next().unwrap();
    if !first.is_ascii_alphabetic() {
        return Err(ParseError {
            line,
            message: format!("flag name '{}' must start with a letter", name),
        });
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(ParseError {
            line,
            message: format!("flag name '{}' contains invalid characters", name),
        });
    }
    Ok(())
}

fn parse_flag_body(
    name: String,
    header_line: usize,
    body: Vec<(usize, String)>,
) -> Result<Flag, ParseError> {
    let mut description = None;
    let mut enabled: Option<bool> = None;
    let mut rollout: Option<u8> = None;
    let mut default: Option<bool> = None;
    let mut rules = Vec::new();

    for (line_no, raw) in &body {
        let line_no = *line_no;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("rule ") {
            rules.push(parse_rule(rest, line_no)?);
            continue;
        }

        let (key, value) = trimmed.split_once('=').ok_or_else(|| ParseError {
            line: line_no,
            message: format!("cannot parse line '{}'", trimmed),
        })?;
        let key = key.trim();
        let value = value.trim();
        match key {
            "description" => description = Some(parse_string(value, line_no)?),
            "enabled" => enabled = Some(parse_bool(value, line_no)?),
            "default" => default = Some(parse_bool(value, line_no)?),
            "rollout" => {
                let n: i64 = value.parse().map_err(|_| ParseError {
                    line: line_no,
                    message: format!("rollout must be an integer, found '{}'", value),
                })?;
                if !(0..=100).contains(&n) {
                    return Err(ParseError {
                        line: line_no,
                        message: format!("rollout must be between 0 and 100, found {}", n),
                    });
                }
                rollout = Some(n as u8);
            }
            other => {
                return Err(ParseError {
                    line: line_no,
                    message: format!("unknown field '{}'", other),
                });
            }
        }
    }

    let enabled = enabled.ok_or_else(|| ParseError {
        line: header_line,
        message: format!("flag '{}' is missing required field 'enabled'", name),
    })?;

    Ok(Flag {
        name,
        description,
        enabled,
        rollout,
        rules,
        default,
    })
}

fn parse_rule(rest: &str, line_no: usize) -> Result<Rule, ParseError> {
    let (cond, result) = rest.split_once("=>").ok_or_else(|| ParseError {
        line: line_no,
        message: "rule must contain '=>'".to_string(),
    })?;
    let cond = cond.trim();
    let result_bool = parse_bool(result.trim(), line_no)?;

    let (field, op, value_str) = if let Some((f, v)) = cond.split_once("==") {
        (f.trim(), Op::Eq, v.trim())
    } else if let Some((f, v)) = cond.split_once("!=") {
        (f.trim(), Op::Ne, v.trim())
    } else {
        return Err(ParseError {
            line: line_no,
            message: format!("rule condition '{}' must use '==' or '!='", cond),
        });
    };

    if field.is_empty() {
        return Err(ParseError {
            line: line_no,
            message: "rule field name must not be empty".to_string(),
        });
    }

    let value = parse_value(value_str, line_no)?;

    Ok(Rule {
        field: field.to_string(),
        op,
        value,
        result: result_bool,
    })
}

fn parse_value(raw: &str, line_no: usize) -> Result<Value, ParseError> {
    if raw == "true" || raw == "false" {
        return Ok(Value::Bool(raw == "true"));
    }
    if let Ok(n) = raw.parse::<i64>() {
        return Ok(Value::Int(n));
    }
    parse_string(raw, line_no).map(Value::Str)
}

fn parse_bool(raw: &str, line_no: usize) -> Result<bool, ParseError> {
    match raw {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(ParseError {
            line: line_no,
            message: format!("expected 'true' or 'false', found '{}'", other),
        }),
    }
}

fn parse_string(raw: &str, line_no: usize) -> Result<String, ParseError> {
    if raw.len() < 2 || !raw.starts_with('"') || !raw.ends_with('"') {
        return Err(ParseError {
            line: line_no,
            message: format!("expected a quoted string, found '{}'", raw),
        });
    }
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some(other) => {
                    return Err(ParseError {
                        line: line_no,
                        message: format!("unknown escape sequence '\\{}'", other),
                    });
                }
                None => {
                    return Err(ParseError {
                        line: line_no,
                        message: "dangling escape at end of string".to_string(),
                    });
                }
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}
