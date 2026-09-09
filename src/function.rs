use runtime::SegOfs;

use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::ai::Var;

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
                if let Some(jmp) = &instr.jmp {
                    writeln!(w, "@jmp {jmp}")?;
                }
                if let Some(label) = &instr.label {
                    writeln!(w, "@label {label}")?;
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

#[derive(DeserializeFromStr, SerializeDisplay, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum XRef {
    Name(String),
    Addr(SegOfs),
    Block(usize, Option<String>),
}

impl std::str::FromStr for XRef {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let xref = if let Some(label) = s.strip_prefix("'") {
            let label = if let Some((_, block)) = label.split_once('@') {
                block
            } else {
                label
            };
            XRef::Block(
                usize::from_str(label).map_err(|err| format!("xref {:?}: {}", s, err))?,
                None,
            )
        } else if s.contains(':') {
            XRef::Addr(SegOfs::parse(s)?)
        } else {
            XRef::Name(s.to_owned())
        };
        Ok(xref)
    }
}

impl std::fmt::Display for XRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XRef::Addr(segofs) => write!(f, "{segofs}"),
            XRef::Name(name) => f.write_str(name),
            XRef::Block(idx, None) => write!(f, "'{idx}"),
            XRef::Block(idx, Some(label)) => write!(f, "'{label}@{idx}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xref_forms_round_trip() {
        let cases = [
            ("main", XRef::Name("main".into())),
            ("1234:abcd", XRef::Addr(SegOfs::new(0x1234, 0xabcd))),
            ("'0", XRef::Block(0, None)),
            ("'42", XRef::Block(42, None)),
        ];

        for (text, expected) in cases {
            let parsed: XRef = text.parse().unwrap();
            assert!(parsed == expected, "incorrect parsing of {text:?}");
            assert_eq!(parsed.to_string(), text);
            assert_eq!(expected.to_string(), text);
            assert!(expected.to_string().parse::<XRef>().unwrap() == expected);
        }
    }

    #[test]
    fn labeled_xref_round_trip_preserves_block_index() {
        let xref = XRef::Block(42, Some("loop".into()));
        assert_eq!(xref.to_string(), "'loop@42");

        // Labels are display annotations; parsing retains only the block index.
        let parsed: XRef = xref.to_string().parse().unwrap();
        assert!(parsed == XRef::Block(42, None));
        assert_eq!(parsed.to_string(), "'42");
        assert!(parsed.to_string().parse::<XRef>().unwrap() == parsed);
    }
}
