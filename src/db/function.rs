use std::collections::BTreeMap;

use runtime::SegOfs;

use crate::{api::Var, db::XRef, dis};

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct Function {
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

impl Function {
    pub fn load_blocks(&mut self, mem: &[u8]) {
        assert!(self.blocks[0].instrs.is_empty());
        let block_ips = self.blocks.iter().map(|b| b.ip).collect::<Vec<_>>();
        for block in self.blocks.iter_mut() {
            block.instrs = dis::gather_block(mem, block.ip, |ip| {
                ip != block.ip && block_ips.contains(&ip)
            });
        }
    }

    pub fn gather_callees(&self) -> Vec<SegOfs> {
        let mut callees = Vec::new();
        for annot in self.annotations.values() {
            if let Some(jmp) = &annot.jmp {
                match jmp {
                    XRef::External(ip) => {
                        if !callees.contains(ip) {
                            callees.push(*ip);
                        }
                    }
                    _ => {}
                }
            }
        }
        callees.sort();
        callees
    }
}

pub struct Block {
    pub ip: SegOfs,
    pub instrs: Vec<Instr>,
}

impl Block {
    pub fn span(&self) -> std::ops::Range<SegOfs> {
        let end_ofs = self.instrs.last().unwrap().next_ip16();
        self.ip..self.ip.with_ofs(end_ofs)
    }

    pub fn contains_ip(&self, ip: SegOfs) -> bool {
        ip.seg == self.ip.seg && self.span().contains(&ip)
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

pub type Instr = iced_x86::Instruction;

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct Annotation {
    pub comment: Option<String>,
    pub label: Option<String>,
    pub jmp: Option<XRef>,
    #[serde(default)]
    #[serde(with = "serde_mem")]
    pub memory: Option<Result<SegOfs, String>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub assume: Vec<(String, u16)>,
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
        Ok(str.map(|str| SegOfs::parse(str).map_err(|_| str.to_owned())))
    }
}
