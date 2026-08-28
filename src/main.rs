use runtime::SegOfs;
mod ai;

const DOSBOX_SEG: u16 = 0x813;

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Mode {
    Dis(Dis),
    AI(AI),
}

/// disassemble
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "dis")]
struct Dis {
    #[argh(positional)]
    path: String,
}

/// ai
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "ai")]
struct AI {}

/// wip
#[derive(argh::FromArgs)]
struct Args {
    #[argh(subcommand)]
    mode: Mode,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = argh::from_env::<Args>();
    match args.mode {
        Mode::Dis(dis) => {
            load(dis.path);
        }
        Mode::AI(_ai) => {
            ai::call().await?;
        }
    }
    Ok(())
}

fn load(path: String) {
    let mut mem = Vec::<u8>::new();
    let psp_segment = DOSBOX_SEG;
    let load_addr = SegOfs::new(psp_segment + 0x10, 0);

    let dos = {
        let buf = std::fs::read(&path).unwrap();
        println!("loading {path}");
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
    gather_block(&mem, SegOfs::new(cs, dos.header.entry_point));
}

fn gather_block(mem: &[u8], block_addr: SegOfs) -> Vec<iced_x86::Instruction> {
    let mut block = Vec::new();
    let decoder = iced_x86::Decoder::with_ip(
        16,
        &mem[block_addr.abs() as usize..],
        block_addr.ofs as u64,
        iced_x86::DecoderOptions::NONE,
    );
    for instr in decoder.into_iter() {
        println!(
            "{addr} {code}",
            addr = block_addr.with_ofs(instr.ip16()),
            code = instr
        );
        block.push(instr);

        use iced_x86::FlowControl::*;
        match instr.flow_control() {
            Next | Call | Interrupt => {}
            _ => break,
        }
    }
    block
}
