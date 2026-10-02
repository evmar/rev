//! API for working with Functions.

use std::collections::HashSet;

use runtime::SegOfs;

use crate::db::Function;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[schemars(inline)]
pub struct FunctionUpdate {
    /// short function name, a guess at what the function does
    pub name: String,
    /// brief description of function: one-liner summary of what function is for
    pub desc: String,
    /// longer description of function: two or three sentences
    pub details: String,
    /// function parameters, both registers and from stack
    pub parameters: Vec<Var>,
    /// return value, if any
    #[serde(rename = "return")]
    pub ret: Option<Var>,
    /// if it's the target of a local jmp, a name for the label for the instruction
    pub labels: Vec<Annotation>,
    /// comment describing what the instruction or following block does
    pub inline_comments: Vec<Annotation>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, schemars::JsonSchema)]
#[schemars(inline)]
pub struct Annotation {
    pub addr: String,
    pub text: String,
}

impl Annotation {
    fn parse(self) -> anyhow::Result<(SegOfs, String)> {
        let addr = SegOfs::parse(&self.addr)
            .map_err(|err| anyhow::anyhow!("parsing {addr}: {err}", addr = self.addr))?;
        Ok((addr, self.text))
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, ts_rs::TS, schemars::JsonSchema)]
#[ts(export)]
#[schemars(inline)]
pub struct Var {
    /// an identifier name for the variable within the function
    pub name: String,
    /// the parameter's storage, either a register like `cx` or an stack offset like `[sp+4]`
    pub value: String,
    /// concrete type of value as expressed in Rust, e.g. `u32` or `[u8]`.  no higher-level types like fn, just numbers, arrays, or pointers.
    #[serde(rename = "type")]
    pub typ: String,
    /// human-readable description
    pub desc: String,
}

impl FunctionUpdate {
    pub fn schema() -> schemars::Schema {
        schemars::generate::SchemaSettings::default()
            .with(|settings| settings.inline_subschemas = true)
            .into_generator()
            .into_root_schema_for::<FunctionUpdate>()
    }

    pub fn merge_into(self, func: &mut Function) -> anyhow::Result<()> {
        func.name = Some(self.name);
        func.desc = Some(self.desc);
        func.details = Some(self.details);

        func.params = self.parameters;
        func.ret = self.ret;

        let mut instrs: HashSet<SegOfs> = Default::default();
        for block in &func.blocks {
            for instr in &block.instrs {
                instrs.insert(func.ip.with_ofs(instr.iced.ip16()));
            }
        }

        func.annotations.clear();

        for label in self.labels {
            let (addr, label) = label.parse()?;
            if !instrs.contains(&addr) {
                anyhow::bail!("label on nonexistent {addr}");
            }
            func.annotations.entry(addr).or_default().label = Some(label);
        }

        for comment in self.inline_comments {
            let (addr, comment) = comment.parse()?;
            if !instrs.contains(&addr) {
                anyhow::bail!("comment on nonexistent {addr}");
            }
            func.annotations.entry(addr).or_default().comment = Some(comment);
        }

        Ok(())
    }
}

pub fn plain_text(func: &Function) -> String {
    use std::fmt::Write;

    let mut str = String::new();
    let w = &mut str;

    writeln!(
        w,
        "function {}{}",
        func.ip,
        func.name
            .as_ref()
            .map(|name| format!(" ({name})"))
            .unwrap_or_default()
    )
    .ok();

    let width = 72;
    if let Some(desc) = &func.desc {
        writeln!(w, "\n{}", textwrap::fill(desc, width)).ok();
    }
    if let Some(details) = &func.details {
        writeln!(w, "\n{}", textwrap::fill(details, width)).ok();
    }

    fn p(w: &mut String, var: &Var) {
        let Var {
            name,
            value,
            typ,
            desc,
        } = var;
        writeln!(w, "  {value}  {name}: {typ} {desc}").ok();
    }
    for param in func.params.iter() {
        writeln!(w, "\nparameters:").ok();
        p(w, param);
    }
    if let Some(ret) = &func.ret {
        writeln!(w, "\nreturn:").ok();
        p(w, ret);
    }

    writeln!(w, "\n---").ok();

    for block in func.blocks.iter() {
        for instr in block.instrs.iter() {
            let ip = func.ip.with_ofs(instr.iced.ip16());
            if let Some(annot) = func.annotations.get(&ip) {
                if let Some(comment) = &annot.comment {
                    writeln!(w, "; {comment}").ok();
                }
                for (reg, val) in annot.assume.iter() {
                    writeln!(w, "@assume {reg}={val:x}").ok();
                }
                if let Some(label) = &annot.label {
                    writeln!(w, "@label {label}").ok();
                }
                if let Some(jmp) = &annot.jmp {
                    writeln!(w, "@jmp {jmp}").ok();
                }
                if let Some(mem) = &annot.memory {
                    match mem {
                        Ok(addr) => writeln!(w, "@mem {addr}").ok(),
                        Err(err) => writeln!(w, "@mem ; {err}").ok(),
                    };
                }
            }
            writeln!(w, "{ip} {instr}", instr = instr.iced).ok();
        }
        writeln!(w).ok();
    }
    str
}
