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
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub params: Vec<Var>,
    pub ret: Option<Var>,

    #[serde(with = "serde_blocks")]
    pub blocks: Vec<Block>,

    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub annotations: BTreeMap<SegOfs, Annotation>,

    #[serde(skip)]
    pub callers: Option<Vec<XRef>>,
    #[serde(skip)]
    pub callees: Option<Vec<XRef>>,
}

impl Function {
    pub fn disasm(&mut self, mem: &[u8]) {
        if !self.blocks[0].instrs.is_empty() {
            return;
        }
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

#[derive(Default)]
pub struct Instr {
    pub comment: Option<String>,
    pub label: Option<String>,
    pub jmp: Option<XRef>,
    pub memory: Option<Result<SegOfs, String>>,
    pub assume: Option<Vec<(String, u16)>>,
    pub iced: iced_x86::Instruction,
}

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
