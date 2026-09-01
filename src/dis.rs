use crate::db::DB;
use runtime::SegOfs;
use std::{
    collections::{BTreeMap, VecDeque},
    ops::Range,
    path::Path,
};

fn ser_segofs<S>(segofs: &SegOfs, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&format!("{}", segofs))
}

#[derive(serde::Serialize)]
pub struct FunctionMeta {
    #[serde(serialize_with = "ser_segofs")]
    pub ip: SegOfs,
}

pub struct Function {
    pub meta: FunctionMeta,
    pub blocks: Vec<Block>,
}

impl Function {
    fn ser(&self, w: &mut impl std::io::Write) -> anyhow::Result<()> {
        writeln!(w, "{}", toml::to_string(&self.meta)?)?;
        writeln!(w, "---")?;

        for block in self.blocks.iter() {
            for instr in block.instrs.iter() {
                let ip = self.meta.ip.with_ofs(instr.ip16());
                writeln!(w, "{ip} {instr}")?;
            }
            println!();
        }
        Ok(())
    }
}

/// disassemble
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "dis")]
pub struct Args {}

pub fn run(db: &DB, _args: Args) {
    let func = load(&db.exe_path());
    func.ser(&mut std::io::stdout()).unwrap();
}

pub fn load(path: &Path) -> Function {
    const DOSBOX_SEG: u16 = 0x813;
    let mut mem = Vec::<u8>::new();
    let psp_segment = DOSBOX_SEG;
    let load_addr = SegOfs::new(psp_segment + 0x10, 0);

    let dos = {
        println!("loading {}", path.display());
        let buf = std::fs::read(path).unwrap();
        let dos = exe::DOS::parse(&buf).unwrap();
        {
            let data = &buf[dos.image_offset()..];
            mem.resize(load_addr.abs() as usize + data.len(), 0);
            mem[load_addr.abs() as usize..].copy_from_slice(data);
        }
        dos.apply_relocations(load_addr.seg, &mut mem[load_addr.abs() as usize..]);

        dos
    };

    let cs = load_addr.seg + dos.header.initial_cs;
    let blocks = gather(&mem, SegOfs::new(cs, dos.header.entry_point));

    Function {
        meta: FunctionMeta { ip: blocks[0].ip },
        blocks,
    }
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
