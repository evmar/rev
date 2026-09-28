use std::collections::BTreeMap;

use runtime::SegOfs;

use crate::{api::Var, db::XRef};

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct Function {
    pub ip: SegOfs,
    pub len: usize,

    pub name: Option<String>,
    pub desc: Option<String>,
    pub details: Option<String>,
    pub callers: Option<Vec<XRef>>,
    pub callees: Option<Vec<XRef>>,

    pub params: Option<Vec<Var>>,
    pub ret: Option<Var>,

    #[serde(skip)]
    pub blocks: Vec<Block>,
}

impl Function {
    pub fn to_plain_text(&self, w: &mut impl std::fmt::Write) -> anyhow::Result<()> {
        writeln!(w, "{}", toml::to_string(self)?)?;
        writeln!(w, "---")?;

        for block in self.blocks.iter() {
            for instr in block.instrs.iter() {
                let ip = self.ip.with_ofs(instr.iced.ip16());
                if let Some(comment) = &instr.comment {
                    writeln!(w, "; {comment}")?;
                }
                if let Some(assume) = &instr.assume {
                    for (reg, val) in assume.iter() {
                        writeln!(w, "@assume {reg}={val:x}")?;
                    }
                }
                if let Some(label) = &instr.label {
                    writeln!(w, "@label {label}")?;
                }
                if let Some(jmp) = &instr.jmp {
                    writeln!(w, "@jmp {jmp}")?;
                }
                if let Some(mem) = &instr.memory {
                    match mem {
                        Ok(mem) => writeln!(w, "@mem {mem}")?,
                        Err(err) => writeln!(w, "@mem ; {err}")?,
                    }
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
        let mut memory: Option<Result<SegOfs, String>> = None;
        let mut assume = vec![];
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
            } else if let Some(m) = line.strip_prefix("@mem ") {
                memory = Some(
                    SegOfs::parse(m).map_err(|_| format!("{}", m.strip_prefix("; ").unwrap_or(m))),
                );
                continue;
            } else if let Some(l) = line.strip_prefix("@label ") {
                label = Some(l.to_owned());
                continue;
            } else if let Some(a) = line.strip_prefix("@assume ") {
                let Some((reg, val)) = a.split_once('=') else {
                    anyhow::bail!("bad assume {a:?}");
                };
                let val = u16::from_str_radix(val, 16)
                    .map_err(|_| anyhow::anyhow!("bad value {val:?}"))?;
                assume.push((reg.to_owned(), val));
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
                memory: std::mem::take(&mut memory),
                assume: if assume.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut assume))
                },
                iced: instr,
            });
        }
        let last = func.blocks.last().unwrap();
        if last.instrs.is_empty() {
            func.blocks.pop();
        }
        let next_ip = func
            .blocks
            .last()
            .unwrap()
            .instrs
            .last()
            .unwrap()
            .iced
            .next_ip16();
        func.len = (next_ip - func.ip.ofs) as usize;

        Ok(func)
    }

    #[allow(unused)]
    pub fn clone_bare(&self) -> Function {
        Function {
            ip: self.ip,
            len: self.len,
            blocks: self.blocks.iter().map(|block| block.clone_bare()).collect(),
            ..Default::default()
        }
    }
}

pub struct Block {
    pub ip: SegOfs,
    pub instrs: Vec<Instr>,
}

impl Block {
    pub fn span(&self) -> std::ops::Range<SegOfs> {
        let end_ofs = self.instrs.last().unwrap().iced.next_ip16();
        self.ip..self.ip.with_ofs(end_ofs)
    }

    pub fn contains_ip(&self, ip: SegOfs) -> bool {
        ip.seg == self.ip.seg && self.span().contains(&ip)
    }

    pub fn clone_bare(&self) -> Block {
        Block {
            ip: self.ip,
            instrs: self.instrs.iter().map(|instr| instr.clone_bare()).collect(),
        }
    }
}

#[derive(Default)]
pub struct Instr {
    pub comment: Option<String>,
    pub label: Option<String>,
    pub jmp: Option<XRef>,
    pub memory: Option<Result<SegOfs, String>>,
    pub assume: Option<Vec<(String, u16)>>,
    pub iced: iced_x86::Instruction,
}

impl Instr {
    pub fn clone_bare(&self) -> Instr {
        Instr {
            iced: self.iced,
            ..Default::default()
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct Function2 {
    pub ip: SegOfs,
    pub len: usize,

    pub name: Option<String>,
    pub desc: Option<String>,
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub params: Vec<Var>,
    pub ret: Option<Var>,

    #[serde(with = "serde_blocks")]
    pub blocks: Vec<Block>,

    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub annotations: BTreeMap<SegOfs, Annotation>,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct Annotation {
    pub comment: Option<String>,
    pub label: Option<String>,
    pub jmp: Option<XRef>,
    #[serde(with = "serde_mem")]
    pub memory: Option<Result<SegOfs, String>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub assume: Vec<(String, u16)>,
}

impl Function2 {
    pub fn deserialize_old(_mem: &[u8], buf: &str) -> anyhow::Result<Self> {
        let Some((header, body)) = buf.split_once("\n---\n") else {
            anyhow::bail!("missing --- separator")
        };

        let func: Function = toml::from_str(header)?;
        let mut func2 = Function2 {
            ip: func.ip,
            len: func.len,
            name: func.name,
            desc: func.desc,
            details: func.details,
            params: func.params.unwrap_or_default(),
            ret: func.ret,
            blocks: Default::default(),
            annotations: Default::default(),
        };

        let mut annot = Annotation::default();
        let mut block_start = true;
        for (i, line) in body.lines().enumerate() {
            if line.is_empty() {
                block_start = true;
                continue;
            }

            if let Some(c) = line.strip_prefix("; ") {
                annot.comment.get_or_insert_default().push_str(c);
                continue;
            } else if let Some(r) = line.strip_prefix("@jmp ") {
                use std::str::FromStr;
                annot.jmp =
                    Some(XRef::from_str(r).map_err(|err| anyhow::anyhow!("bad xref {r}: {err}"))?);
                continue;
            } else if let Some(m) = line.strip_prefix("@mem ") {
                let m = m.strip_prefix("; ").unwrap_or(m);
                annot.memory = Some(SegOfs::parse(m).map_err(|_| m.to_owned()));
                continue;
            } else if let Some(l) = line.strip_prefix("@label ") {
                annot.label = Some(l.to_owned());
                continue;
            } else if let Some(a) = line.strip_prefix("@assume ") {
                let Some((reg, val)) = a.split_once('=') else {
                    anyhow::bail!("bad assume {a:?}");
                };
                let val = u16::from_str_radix(val, 16)
                    .map_err(|_| anyhow::anyhow!("bad value {val:?}"))?;
                annot.assume.push((reg.to_owned(), val));
                continue;
            }

            let Some((addr, _)) = line.split_once(' ') else {
                anyhow::bail!("{i}: {line:?} missing addr")
            };
            let addr = SegOfs::parse(addr).map_err(|err| anyhow::anyhow!("{i}: {addr:?} {err}"))?;

            if block_start {
                func2.blocks.push(Block {
                    ip: addr,
                    instrs: vec![],
                });
                block_start = false;
            }

            if annot.comment.is_none()
                && annot.label.is_none()
                && annot.jmp.is_none()
                && annot.memory.is_none()
                && annot.assume.is_empty()
            {
                continue;
            }

            func2.annotations.insert(addr, annot);
            annot = Annotation::default();
        }

        Ok(func2)
    }

    pub fn disasm(&mut self, mem: &[u8]) {
        assert!(self.blocks[0].instrs.is_empty());
        let blocks = self.blocks.iter().map(|b| b.ip).collect::<Vec<_>>();
        for block in self.blocks.iter_mut() {
            Self::disasm_block(mem, &blocks, block);
        }
    }

    fn disasm_block(mem: &[u8], blocks: &[SegOfs], block: &mut Block) {
        // TODO: share code with dis?
        let mut instrs = Vec::new();
        let decoder = iced_x86::Decoder::with_ip(
            16,
            &mem[block.ip.abs() as usize..],
            block.ip.ofs as u64,
            iced_x86::DecoderOptions::NONE,
        );
        for instr in decoder.into_iter() {
            if !instrs.is_empty() && blocks.contains(&block.ip.with_ofs(instr.ip16())) {
                break;
            }

            instrs.push(Instr {
                iced: instr,
                ..Default::default()
            });

            use iced_x86::FlowControl::*;
            match instr.flow_control() {
                Next | Call | IndirectCall | Interrupt => {
                    // assume it returns
                }
                IndirectBranch | UnconditionalBranch | ConditionalBranch | Return => break,
                XbeginXabortXend | Exception => todo!(),
            }
        }
        block.instrs = instrs;
    }
}

mod serde_blocks {
    use super::*;
    pub fn serialize<S: serde::Serializer>(blocks: &Vec<Block>, ser: S) -> Result<S::Ok, S::Error> {
        ser.collect_seq(blocks.iter().map(|b| b.ip))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(deser: D) -> Result<Vec<Block>, D::Error> {
        let ips: Vec<SegOfs> = serde::Deserialize::deserialize(deser)?;
        Ok(ips
            .into_iter()
            .map(|ip| Block { ip, instrs: vec![] })
            .collect())
    }
}

mod serde_mem {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        mem: &Option<Result<SegOfs, String>>,
        ser: S,
    ) -> Result<S::Ok, S::Error> {
        match mem {
            Some(mem) => match mem {
                Ok(addr) => ser.serialize_str(&format!("{}", addr)),
                Err(str) => ser.serialize_str(str),
            },
            None => ser.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deser: D,
    ) -> Result<Option<Result<SegOfs, String>>, D::Error> {
        let str: Option<&str> = serde::Deserialize::deserialize(deser)?;
        Ok(str.map(SegOfs::parse))
    }
}
