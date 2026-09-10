use runtime::SegOfs;

use crate::{ai::Var, xref::XRef};

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct Function {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub details: Option<String>,
    pub xrefs: Option<Vec<XRef>>,

    pub params: Option<Vec<Var>>,
    pub ret: Option<Var>,

    pub ip: SegOfs,

    #[serde(skip)]
    pub blocks: Vec<Block>,
}

impl Function {
    pub fn serialize(&self, w: &mut impl std::io::Write) -> anyhow::Result<()> {
        writeln!(w, "{}", toml::to_string(self)?)?;
        writeln!(w, "---")?;

        for block in self.blocks.iter() {
            for instr in block.instrs.iter() {
                let ip = self.ip.with_ofs(instr.iced.ip16());
                if let Some(comment) = &instr.comment {
                    writeln!(w, "; {comment}")?;
                }
                if let Some(label) = &instr.label {
                    writeln!(w, "@label {label}")?;
                }
                if let Some(jmp) = &instr.jmp {
                    writeln!(w, "@jmp {jmp}")?;
                }
                writeln!(w, "{ip} {instr}", instr = instr.iced)?;
            }
            writeln!(w)?;
        }
        Ok(())
    }

    pub fn deserialize(mem: &[u8], buf: &str) -> anyhow::Result<Self> {
        let Some((header, body)) = buf.split_once("\n---\n") else {
            anyhow::bail!("missing --- separator")
        };

        let mut func: Function = toml::from_str(header)?;

        let mut decoder = iced_x86::Decoder::new(16, &mem, iced_x86::DecoderOptions::NONE);
        let mut block = func.blocks.push_mut(Block {
            ip: Default::default(),
            instrs: vec![],
        });
        let mut comment = String::new();
        let mut label = None;
        let mut jmp = None;
        for (i, line) in body.lines().enumerate() {
            if line.is_empty() {
                block = func.blocks.push_mut(Block {
                    ip: Default::default(),
                    instrs: vec![],
                });
                continue;
            }

            if let Some(c) = line.strip_prefix("; ") {
                comment.push_str(c);
                continue;
            } else if let Some(r) = line.strip_prefix("@jmp ") {
                use std::str::FromStr;
                jmp =
                    Some(XRef::from_str(r).map_err(|err| anyhow::anyhow!("bad xref {r}: {err}"))?);
                continue;
            } else if let Some(l) = line.strip_prefix("@label ") {
                label = Some(l.to_owned());
                continue;
            }

            let Some((addr, _)) = line.split_once(' ') else {
                anyhow::bail!("{i}: {line:?} missing addr")
            };
            let addr = SegOfs::parse(addr).map_err(|err| anyhow::anyhow!("{i}: {addr:?} {err}"))?;

            if block.ip.is_null() {
                block.ip = addr;
            }

            decoder.set_ip(addr.ofs as u64);
            decoder.set_position(addr.abs() as usize).unwrap();
            let instr = decoder.decode();
            block.instrs.push(Instr {
                comment: if comment.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut comment))
                },
                label: std::mem::take(&mut label),
                jmp: std::mem::take(&mut jmp),
                iced: instr,
            });
        }
        let last = func.blocks.last().unwrap();
        if last.instrs.is_empty() {
            func.blocks.pop();
        }

        Ok(func)
    }
}

pub struct Block {
    pub ip: SegOfs,
    pub instrs: Vec<Instr>,
}

impl Block {
    pub fn span(&self) -> std::ops::Range<SegOfs> {
        self.ip..self.ip.with_ofs(self.instrs.last().unwrap().iced.ip16())
    }

    pub fn contains_ip(&self, ip: SegOfs) -> bool {
        ip.seg == self.ip.seg && self.span().contains(&ip)
    }
}

#[derive(Default)]
pub struct Instr {
    pub comment: Option<String>,
    pub label: Option<String>,
    pub jmp: Option<XRef>,
    pub iced: iced_x86::Instruction,
}
