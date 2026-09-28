use std::path::PathBuf;

use crate::{
    api::Var,
    db::{DB, Function2},
};

/// dump program database as text
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "dump")]
pub struct Args {
    /// directory to dump to
    #[argh(positional)]
    out_dir: PathBuf,
}

pub fn dump(db: &mut DB, Args { out_dir }: Args) -> anyhow::Result<()> {
    std::fs::create_dir_all(&out_dir)?;

    std::fs::write(
        out_dir.join("db.toml"),
        toml::to_string(&db.exe)?.as_bytes(),
    )?;
    std::fs::write(
        out_dir.join("memory.toml"),
        toml::to_string(&db.memory)?.as_bytes(),
    )?;

    let fn_dir = out_dir.join("fn");
    std::fs::create_dir_all(&fn_dir)?;
    for func in db.functions2.values_mut() {
        func.disasm(&db.mem);
        let name = match &func.name {
            Some(name) => format!("{name}.toml"),
            None => format!("{:04x}_{:04x}.toml", func.ip.seg, func.ip.ofs),
        };
        let path = fn_dir.join(name);
        std::fs::write(&path, plain_text(func)?.as_bytes())?;
    }

    Ok(())
}

fn plain_text(func: &Function2) -> anyhow::Result<String> {
    let mut str = String::new();
    let w = &mut str;
    use std::fmt::Write;

    writeln!(w, "ip = \"{}\"", func.ip)?;
    writeln!(w, "len = {}", func.len)?;
    if let Some(name) = &func.name {
        writeln!(w, "name = {:?}", name)?;
    }
    if let Some(desc) = &func.desc {
        writeln!(w, "desc = {:?}", desc)?;
    }
    if let Some(details) = &func.details {
        writeln!(w, "details = {:?}", details)?;
    }

    fn p(w: &mut String, name: &str, var: &Var) -> anyhow::Result<()> {
        writeln!(w)?;
        writeln!(w, "[{name}]")?;
        writeln!(w, "name = {:?}", var.name)?;
        writeln!(w, "value = {:?}", var.value)?;
        writeln!(w, "type = {:?}", var.typ)?;
        writeln!(w, "desc = {:?}", var.desc)?;
        Ok(())
    }
    for param in func.params.iter() {
        p(w, "[params]", param)?;
    }
    if let Some(ret) = &func.ret {
        p(w, "ret", ret)?;
    }

    writeln!(w, "\n---")?;

    for block in func.blocks.iter() {
        for instr in block.instrs.iter() {
            let ip = func.ip.with_ofs(instr.iced.ip16());
            if let Some(annot) = func.annotations.get(&ip) {
                if let Some(comment) = &annot.comment {
                    writeln!(w, "; {comment}")?;
                }
                for (reg, val) in annot.assume.iter() {
                    writeln!(w, "@assume {reg}={val:x}")?;
                }
                if let Some(label) = &annot.label {
                    writeln!(w, "@label {label}")?;
                }
                if let Some(jmp) = &annot.jmp {
                    writeln!(w, "@jmp {jmp}")?;
                }
                if let Some(mem) = &annot.memory {
                    match mem {
                        Ok(addr) => writeln!(w, "@mem {addr}")?,
                        Err(err) => writeln!(w, "@mem ; {err}")?,
                    }
                }
            }
            writeln!(w, "{ip} {instr}", instr = instr.iced)?;
        }
        writeln!(w)?;
    }
    Ok(str)
}
