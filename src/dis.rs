use crate::db::DB;
use runtime::SegOfs;
use std::{
    collections::{BTreeMap, VecDeque},
    ops::Range,
};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Function {
    pub name: Option<String>,

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
                let ip = self.ip.with_ofs(instr.ip16());
                writeln!(w, "{ip} {instr}")?;
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
        for (i, line) in body.lines().enumerate() {
            if line.is_empty() {
                block = func.blocks.push_mut(Block {
                    ip: Default::default(),
                    instrs: vec![],
                });
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
            block.instrs.push(instr);
        }

        Ok(func)
    }
}

/// disassemble
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "dis")]
pub struct Args {
    #[argh(positional, from_str_fn(SegOfs::parse))]
    addr: SegOfs,
}

pub fn run(db: &mut DB, args: Args) -> anyhow::Result<()> {
    let ip = args.addr;
    let func = match db.functions.get_mut(&ip) {
        Some(func) => func,
        None => {
            let func = Function {
                name: None,
                ip,
                blocks: Default::default(),
            };
            db.functions.insert(ip, func);
            db.functions.get_mut(&ip).unwrap()
        }
    };
    check_coverage(&func);

    func.blocks = gather(&db.mem, ip);

    db.write()?;
    Ok(())
}

fn check_coverage(func: &Function) {
    let start = func.blocks[0].instrs[0].ip16() as usize;
    let mut covered = vec![];
    for block in func.blocks.iter() {
        for instr in block.instrs.iter() {
            let end = instr.ip16() as usize + instr.len();
            covered.resize(end - start, false);
            covered[instr.ip16() as usize - start..][..instr.len()].fill(true);
        }
    }

    let count = covered.iter().filter(|&&b| b).count();
    println!(
        "{count}/{len} bytes covered",
        count = count,
        len = covered.len()
    );
}

fn gather(mem: &[u8], start: SegOfs) -> Vec<Block> {
    let mut queue = VecDeque::new();
    let mut blocks = BTreeMap::<SegOfs, Block>::new();
    queue.push_back(start);
    while let Some(ip) = queue.pop_front() {
        if let Some((&baddr, block)) = blocks.range(..=ip).last() {
            if baddr == ip {
                // already visited
                continue;
            }
            if block.contains_ip(ip) {
                blocks.remove(&baddr);
                queue.push_back(baddr);
            }
        }

        let block = gather_block(
            mem,
            ip,
            |ip| blocks.contains_key(&ip),
            |ip| queue.push_back(ip),
        );
        blocks.insert(ip, block);
    }
    blocks.into_values().collect()
}

pub struct Block {
    ip: SegOfs,
    instrs: Vec<iced_x86::Instruction>,
}

impl Block {
    fn span(&self) -> Range<SegOfs> {
        self.ip..self.ip.with_ofs(self.instrs.last().unwrap().ip16())
    }

    fn contains_ip(&self, ip: SegOfs) -> bool {
        ip.seg == self.ip.seg && self.span().contains(&ip)
    }
}

fn gather_block(
    mem: &[u8],
    block_ip: SegOfs,
    visited: impl Fn(SegOfs) -> bool,
    mut enqueue: impl FnMut(SegOfs),
) -> Block {
    let mut instrs = Vec::new();
    let decoder = iced_x86::Decoder::with_ip(
        16,
        &mem[block_ip.abs() as usize..],
        block_ip.ofs as u64,
        iced_x86::DecoderOptions::NONE,
    );
    for instr in decoder.into_iter() {
        if visited(block_ip.with_ofs(instr.ip16())) {
            assert!(!instrs.is_empty());
            break;
        }

        // println!(
        //     "{addr} {code}",
        //     addr = block_ip.with_ofs(instr.ip16()),
        //     code = instr
        // );
        instrs.push(instr);

        use iced_x86::FlowControl::*;
        match instr.flow_control() {
            Next => {}
            Call | IndirectCall | Interrupt => {
                // assume it returns
            }

            IndirectBranch | UnconditionalBranch | ConditionalBranch => {
                control_flow(block_ip, &instr, &mut enqueue);
                if instr.flow_control() == ConditionalBranch {
                    enqueue(block_ip.with_ofs(instr.next_ip16()));
                }
                break;
            }

            Return => break,

            XbeginXabortXend | Exception => todo!(),
        }
    }
    Block {
        ip: block_ip,
        instrs,
    }
}

fn control_flow(ip: SegOfs, instr: &iced_x86::Instruction, mut enqueue: impl FnMut(SegOfs)) {
    use iced_x86::OpKind::*;
    assert_eq!(instr.op_count(), 1);
    match instr.op0_kind() {
        NearBranch16 => enqueue(ip.with_ofs(instr.near_branch16())),
        FarBranch16 => {
            enqueue((instr.far_branch_selector(), instr.far_branch16()).into());
        }
        Memory => {}
        Register => {
            // jmp [reg]  for some register
            // log::warn!("{ip} {instr}  ; indirect via register");
        }
        d => todo!("unhandled jmp {d:?}"),
    }
}
