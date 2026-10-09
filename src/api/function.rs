//! API for working with Functions.

use std::collections::HashSet;

use runtime::SegOfs;

use crate::db::{DB, Function, XRef};

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
    pub addr: SegOfs,
    pub text: String,
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
                instrs.insert(func.ip.with_ofs(instr.ip16()));
            }
        }

        func.annotations.clear();

        for Annotation { addr, text } in self.labels {
            if !instrs.contains(&addr) {
                anyhow::bail!("label on nonexistent {addr}");
            }
            func.annotations.entry(addr).or_default().label = Some(text);
        }

        for Annotation { addr, text } in self.inline_comments {
            if !instrs.contains(&addr) {
                anyhow::bail!("comment on nonexistent {addr}");
            }
            func.annotations.entry(addr).or_default().comment = Some(text);
        }

        Ok(())
    }
}

pub fn plain_text(db: &DB, func: &Function, include_body: bool) -> String {
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

    if let Some(callers) = db.xrefs.callers.get(&func.ip) {
        writeln!(w, "called by:").ok();
        for addr in callers {
            if let Some(name) = db.functions.get(&addr).and_then(|func| func.name.as_ref()) {
                writeln!(w, "  {addr} ({name})").ok();
            } else {
                writeln!(w, "  {addr}").ok();
            }
        }
    }

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

    if !include_body {
        return str;
    }

    writeln!(w, "\n---").ok();

    let block_labels = func.block_labels();
    for (label, block) in block_labels.iter().zip(func.blocks.iter()) {
        if let Some(label) = label {
            writeln!(w, "{label}:").ok();
        }
        let mut post_comment = None;
        for instr in block.instrs.iter() {
            let ip = func.ip.with_ofs(instr.ip16());
            if let Some(annot) = func.annotations.get(&ip) {
                if let Some(comment) = &annot.comment {
                    writeln!(w, "; {comment}").ok();
                }
                for (reg, val) in annot.assume.iter() {
                    writeln!(w, "; assume {reg}={val:x}").ok();
                }
                if let Some(mem) = &annot.memory {
                    let label = match mem {
                        Ok(addr) => match db.memory.entries.get(addr).map(|m| &m.name) {
                            Some(name) => format!("{addr} ({name})"),
                            None => format!("{addr}"),
                        },
                        Err(err) => err.clone(),
                    };
                    post_comment = Some(format!("memory: {label}"));
                }
                if let Some(jmp) = &annot.jmp {
                    let target = match jmp {
                        XRef::External(addr) => {
                            if let Some(name) =
                                db.functions.get(&addr).and_then(|func| func.name.as_ref())
                            {
                                format!("fn {name}()")
                            } else {
                                format!("{addr}")
                            }
                        }
                        XRef::Block(i) => format!("label {}:", block_labels[*i].as_ref().unwrap()),
                        XRef::Unknown(err) => format!("[{err}]"),
                    };
                    post_comment = Some(format!("=> {target}"));
                }
            }

            write!(w, "{ip} {instr}").ok();
            if let Some(c) = std::mem::take(&mut post_comment) {
                write!(w, " ; {c}").ok();
            };
            writeln!(w).ok();
        }
        writeln!(w).ok();
    }
    str
}
