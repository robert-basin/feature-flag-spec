# flagspec

A small text format for describing feature flags, plus a validating parser
and a pretty printer for it. Standard library only, no dependencies.

## The problem

Feature flag config tends to end up in one of two bad places: a giant JSON
blob nobody can diff sensibly, or a bespoke format that every internal tool
parses slightly differently. flagspec is an attempt at a middle ground: a
format specific enough to feature flags that a parser can actually validate
it (rollout must be 0-100, rules must resolve to a boolean, flag names must
be identifiers), and plain enough that a diff in a pull request reads like
English.

The other thing config files like this tend to get wrong is memory: a
"just read the whole file into a String and parse it" implementation works
fine until someone points it at a flag file with 200,000 entries generated
by some other system, at which point it falls over or gets slow. flagspec's
parser reads one `flag { ... }` block at a time from a `BufRead` and never
holds more than the current block in memory, so it scales the same way
whether the file has ten flags or ten million.

## Format

```
# comments start with a hash and run to the end of the line

flag checkout.new_ui {
    description = "Show the redesigned checkout page"
    enabled = true
    rollout = 25
    rule region == "eu" => true
    rule plan != "free" => true
    default = false
}

flag search.v2 {
    enabled = false
}
```

A file is a sequence of `flag <name> { ... }` blocks. Inside a block:

- `description = "..."` — optional, a quoted string.
- `enabled = true|false` — required.
- `rollout = <0-100>` — optional integer percentage.
- `rule <field> ==|!= <value> => true|false` — zero or more. `<value>` is a
  quoted string, an integer, or `true`/`false`.
- `default = true|false` — optional fallback when no rule matches.
- the closing `}` must be on its own line.

Flag names must start with a letter and contain only letters, digits, `.`,
`_`, and `-`.

## Usage

As a library:

```rust
use feature_flag_spec::parser::FlagReader;
use std::fs::File;
use std::io::BufReader;

let file = File::open("flags.txt")?;
let reader = FlagReader::new(BufReader::new(file));

for flag in reader {
    let flag = flag?;
    println!("{}: enabled={}", flag.name, flag.enabled);
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

As a CLI, build the `flagspec` binary and point it at a file:

```
$ flagspec flags.txt
ok: 2 flag(s)

$ flagspec --pretty flags.txt
flag checkout.new_ui {
    description = "Show the redesigned checkout page"
    enabled = true
    rollout = 25
    rule region == "eu" => true
    rule plan != "free" => true
    default = false
}
flag search.v2 {
    enabled = false
}
```

On a validation error the CLI prints the file path and line number and
exits non-zero, for example:

```
$ flagspec bad.txt
bad.txt: line 4: rollout must be between 0 and 100, found 150
```

## Streaming

`FlagReader` implements `Iterator<Item = Result<Flag, FlagError>>` over any
`BufRead`. It reads line by line, accumulating only the lines that belong
to the block currently being parsed. As soon as a block's closing `}` is
seen, that buffer is parsed into a `Flag`, handed to the caller, and
dropped. `printer::write_flag` mirrors this on the way out: it writes one
flag at a time to any `Write`, so a full read-validate-reprint pipeline
never materializes the whole file in either direction.

## Status

This is a first pass at the format and the parser. It covers the grammar
above; things like nested rule groups, multi-line strings, and a `--check`
mode that diffs a file against its own canonical formatting are not there
yet.

## License

MIT, see LICENSE.
