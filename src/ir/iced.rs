//! Conversion from iced_x86 types to ir types.
//!
//! Includes the main "x86 opcode to AST statement" logic.

use super::ast::{Call, Expr, Name, Stmt, StmtKind};

fn name_from_iced(instr: &iced_x86::Instruction, op: u32) -> Name {
    use iced_x86::OpKind::*;
    match instr.op_kind(op) {
        Register => Name::new(format!("{:?}", instr.op_register(op)).to_ascii_lowercase()),
        k => todo!("{k:?}"),
    }
}

impl Expr {
    fn from_memory(instr: &iced_x86::Instruction) -> Self {
        let mut args = Vec::new();
        let seg = instr.memory_segment();
        let seg = match seg {
            iced_x86::Register::CS
            | iced_x86::Register::DS
            | iced_x86::Register::ES
            | iced_x86::Register::FS
            | iced_x86::Register::GS
            | iced_x86::Register::SS => Expr::from(seg),
            iced_x86::Register::None => Expr::from("ds"),
            r => todo!("{r:?}"),
        };

        match instr.memory_base() {
            iced_x86::Register::None => {}
            r => args.push(Expr::from(r)),
        }

        if instr.memory_index() != iced_x86::Register::None {
            let mut expr = Expr::from(instr.memory_index());
            if instr.memory_index_scale() != 1 {
                expr = Expr::Call(Box::new(Call {
                    func: "*".into(),
                    args: vec![expr, Expr::Val(instr.memory_index_scale())],
                }));
            }
            args.push(expr);
        }

        let offset = instr.memory_displacement32();
        if offset != 0 {
            args.push(Expr::Val(offset));
        }

        let sum = match args.len() {
            0 => 0.into(),
            1 => args.pop().unwrap(),
            _ => Expr::call("+", args),
        };
        Expr::call("@", vec![Expr::call("segofs", vec![seg, sum])])
    }

    fn from_iced(instr: &iced_x86::Instruction, op: u32) -> Self {
        use iced_x86::OpKind::*;
        match instr.op_kind(op) {
            Immediate8 => Expr::Val(instr.immediate8() as u32),
            Immediate8to16 => Expr::Val(instr.immediate8to16() as u32),
            Immediate8to32 => Expr::Val(instr.immediate8to32() as u32),
            Immediate16 => Expr::Val(instr.immediate16() as u32),
            Immediate32 => Expr::Val(instr.immediate32()),
            NearBranch16 => Expr::Val(instr.near_branch16() as u32),
            NearBranch32 => Expr::Val(instr.near_branch32()),
            Register => Expr::Reg(name_from_iced(instr, op)),
            Memory => Self::from_memory(instr),
            k => todo!("{k:?}"),
        }
    }
}

impl From<iced_x86::Register> for Expr {
    fn from(reg: iced_x86::Register) -> Self {
        format!("{reg:?}").to_ascii_lowercase().into()
    }
}

impl TryFrom<&iced_x86::Instruction> for Stmt {
    type Error = String;
    fn try_from(instr: &iced_x86::Instruction) -> Result<Self, Self::Error> {
        Ok(Stmt {
            ip: vec![instr.ip32()],
            kind: StmtKind::try_from(instr)?,
        })
    }
}

impl TryFrom<&iced_x86::Instruction> for StmtKind {
    type Error = String;

    fn try_from(instr: &iced_x86::Instruction) -> Result<Self, Self::Error> {
        use iced_x86::Mnemonic::*;
        let mnemonic = instr.mnemonic();
        let stmt = match mnemonic {
            Mov => {
                let var = Expr::from_iced(instr, 0);
                let expr = Expr::from_iced(instr, 1);
                StmtKind::Set(var, expr)
            }
            Inc | Dec => {
                let expr = Expr::from_iced(instr, 0);
                let bin = self::Call {
                    func: match mnemonic {
                        Inc => "+".into(),
                        Dec => "-".into(),
                        _ => unreachable!(),
                    },
                    args: vec![expr.clone(), Expr::Val(1)],
                };
                StmtKind::Set(expr, Expr::from(bin))
            }
            Cmp | Test => {
                let left = Expr::from_iced(instr, 0);
                let right = Expr::from_iced(instr, 1);
                let func = match mnemonic {
                    Cmp => "cmp",
                    Test => "test",
                    _ => unreachable!(),
                }
                .into();
                let bin = self::Call {
                    func,
                    args: vec![left, right],
                };
                StmtKind::Do(Expr::from(bin))
            }
            Add | Shl | Sub | Xor | Sar | And => {
                let left = Expr::from_iced(instr, 0);
                let right = Expr::from_iced(instr, 1);
                let func = match mnemonic {
                    Add => "+",
                    Shl => "<<",
                    Sar => ">>",
                    Sub => "-",
                    Xor => "^",
                    And => "&",
                    _ => unreachable!(),
                }
                .into();
                let bin = self::Call {
                    func,
                    args: vec![left.clone(), right],
                };
                StmtKind::Set(left, Expr::from(bin))
            }
            Lea => {
                let left = Expr::from_iced(instr, 0);
                let right = Expr::from_iced(instr, 1);
                let Expr::Call(call) = right else {
                    unreachable!()
                };
                assert_eq!(call.func, "@");
                let [addr] = call.args.as_slice() else {
                    unreachable!()
                };
                StmtKind::Set(left, addr.clone())
            }
            Jmp | Jae | Jb | Je | Jge | Jne | Jle | Jl => {
                let cond = Box::new(self::Call {
                    func: format!("{mnemonic:?}").to_ascii_lowercase(),
                    args: vec![],
                });
                let dst = Expr::from_iced(instr, 0);
                StmtKind::Jmp(cond, dst)
            }
            Jcxz => {
                let cond = Box::new(self::Call {
                    func: "=".into(),
                    args: vec!["cx".to_owned().into(), 0.into()],
                });
                let dst = Expr::from_iced(instr, 0);
                StmtKind::Jmp(cond, dst)
            }
            Ret | Retf => {
                let dst = Expr::Todo("stack ref".into());
                StmtKind::Jmp(
                    Box::new(self::Call {
                        func: format!("{mnemonic:?}").to_ascii_lowercase(),
                        args: vec![],
                    }),
                    dst,
                )
            }
            Not | Neg => {
                let expr = Expr::from_iced(instr, 0);
                let bin = self::Call {
                    func: match mnemonic {
                        Not => "!".into(),
                        Neg => "-".into(),
                        _ => unreachable!(),
                    },
                    args: vec![expr.clone()],
                };
                StmtKind::Set(expr, Expr::from(bin))
            }
            Call => {
                let expr = Expr::from_iced(instr, 0);
                let call = self::Call {
                    func: "call".into(),
                    args: vec![expr.clone()],
                };
                StmtKind::Do(call.into())
            }
            Cdq => StmtKind::Set(
                "edx".into(),
                self::Call {
                    func: "sign-extend".into(),
                    args: vec!["eax".into()],
                }
                .into(),
            ),
            Push => {
                let expr = Expr::from_iced(instr, 0);
                let call = self::Call {
                    func: "push".into(),
                    args: vec![expr.clone()],
                };
                StmtKind::Do(call.into())
            }
            Pop => StmtKind::Set(
                Expr::from_iced(instr, 0),
                self::Call {
                    func: "pop".into(),
                    args: vec![],
                }
                .into(),
            ),
            Cli | Sti | Cld => StmtKind::Do(
                self::Call {
                    func: format!("{mnemonic:?}").to_ascii_lowercase(),
                    args: vec![],
                }
                .into(),
            ),
            Int => {
                assert_eq!(instr.op_count(), 1);
                StmtKind::Do(
                    self::Call {
                        func: "int".into(),
                        args: vec![Expr::from_iced(instr, 0)],
                    }
                    .into(),
                )
            }
            Imul | Idiv | Stosb | Nop => StmtKind::Raw(format!("{}", instr)),
            m => return Err(format!("{m:?} in {instr}")),
        };
        Ok(stmt)
    }
}
