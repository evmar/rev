//! Function write API.

use std::collections::HashMap;

use runtime::SegOfs;

use crate::function::{Function, Instr};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
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

#[derive(serde::Deserialize, Debug, schemars::JsonSchema)]
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

        func.params = if self.parameters.is_empty() {
            None
        } else {
            Some(self.parameters)
        };
        func.ret = self.ret;

        let mut instrs: HashMap<SegOfs, &mut Instr> = Default::default();
        for block in func.blocks.iter_mut() {
            for instr in block.instrs.iter_mut() {
                instr.label = None;
                instr.comment = None;
                instrs.insert(func.ip.with_ofs(instr.iced.ip16()), instr);
            }
        }

        for label in self.labels {
            let (addr, label) = label.parse()?;
            let Some(instr) = instrs.get_mut(&addr) else {
                anyhow::bail!("label on nonexistent {addr}");
            };
            instr.label = Some(label);
        }

        for comment in self.inline_comments {
            let (addr, comment) = comment.parse()?;
            let Some(instr) = instrs.get_mut(&addr) else {
                anyhow::bail!("comment on nonexistent {addr}");
            };
            instr.comment = Some(comment);
        }

        Ok(())
    }
}
