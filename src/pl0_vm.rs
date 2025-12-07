use std::fmt::{Debug, Display};
use std::io::{stdin, BufRead};
use crate::opcodes::OpCode;
use crate::pl0_vm::Data::{B16, B32, B64};

const ARG_SIZE: usize = 2;
const HEX_ARG_SIZE: usize = ARG_SIZE * 2;
const DEBUG: bool = false;

#[derive(Debug)]
struct Procedure {
    // byte position of procedure in program
    start_pos: usize,
    // starts with space for variables
    frame_ptr: usize,
}

#[derive(Debug, Clone)]
enum Data {
    B16(i16),
    B32(i32),
    B64(i64),
}
impl Data {
    fn i64(&self) -> i64 {
        <Self as Into<i64>>::into(self.clone())
    }
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            B16(x) => x.to_le_bytes().to_vec(),
            B32(x) => x.to_le_bytes().to_vec(),
            B64(x) => x.to_le_bytes().to_vec(),
        }
    }
}
impl Into<i64> for Data {
    fn into(self) -> i64 {
        match self {
            B16(num) => num as i64,
            B32(num) => num as i64,
            B64(num) => num,
        }
    }
}

pub struct PL0VM {
    program: Vec<u8>,
    bits: Data,
}

impl PL0VM {
    pub fn new() -> PL0VM { PL0VM {
        program: vec![],
        bits: Data::B16(0),
    } }
    fn data_size(&self) -> usize { match self.bits { Data::B16(_) => 2, Data::B32(_) => 4, Data::B64(_) => 8 } }

    pub fn from_file(filename: &str) -> Result<PL0VM, std::io::Error> {
        let mut pl0vm = PL0VM::new();
        match pl0vm.load_from_file(filename) {
            Ok(_) => Ok(pl0vm),
            Err(e) => Err(e),
        }
    }

    pub fn load_from_file(&mut self, filename: &str) -> Result<bool, std::io::Error> {
        match std::fs::read(filename) {
            Ok(bytes) => {
                self.program = bytes;
                self.bits = match self.read_arg(ARG_SIZE) {
                    2 => B16(0),
                    4 => B32(0),
                    8 => B64(0),
                    _ => panic!("invalid architecture byte!"),
                };
                Ok(true)
            },
            Err(err) => { Err(err) },
        }
    }

    fn read_arg(&self, offset: usize) -> i16 {
        i16::from_le_bytes(self.program[offset..(offset + ARG_SIZE)].try_into().expect("Invalid byte count?!"))
    }
    fn bytes_to_data(&self, bytes: &[u8]) -> Data {
        match self.bits {
            B16(_) => B16(i16::from_le_bytes(bytes[0..2].try_into().expect("Invalid byte count?!"))),
            B32(_) => B32(i32::from_le_bytes(bytes[0..4].try_into().expect("Invalid byte count?!"))),
            B64(_) => B64(i64::from_le_bytes(bytes[0..8].try_into().expect("Invalid byte count?!"))),
        }
    }
    fn read_data(&self, offset: usize) -> Data {
        self.bytes_to_data(&self.program[offset..])
    }

    pub fn debug_print(&self) {
        let mut pc = 4;
        let mut procedure_count = self.read_arg(0);
        println!("Procedure count: {}", procedure_count);
        print!("Arch: ");
        match self.read_arg(ARG_SIZE) {
            2 => println!("16 bit"),
            4 => println!("32 bit"),
            8 => println!("64 bit"),
            _ => (),
        }

        let print_arg = |pc: &mut usize, last: bool| {
            print!("{:0HEX_ARG_SIZE$X}{}", self.read_arg(*pc), if last { "" } else { ", " });
            *pc += ARG_SIZE;
        };

        let mut rem_bytes = 0;
        loop {
            let byte = self.program[pc];
            let opc = pc;
            let op = OpCode::try_from(byte).expect("Unknown opcode");
            print!("{:04X}: {:02X} {:<20} ", pc - 4, byte, op);
            pc += 1;
            match op {
                OpCode::PushValueLocalVar | OpCode::PushValueMainVar
                    | OpCode::PushAddressLocalVar | OpCode::PushAddressMainVar
                    | OpCode::CallProc | OpCode::PushConstant => {
                    print_arg(&mut pc, true);
                },
                OpCode::Jump | OpCode::JumpIfFalse => {
                    let arg = self.read_arg(pc);
                    print!("{}{:0HEX_ARG_SIZE$X}", if arg < 0 { "-" } else { "" }, arg.abs());
                    pc += ARG_SIZE;
                },
                OpCode::PushValueGlobalVar | OpCode::PushAddressGlobalVar => {
                    print_arg(&mut pc, false);
                    print_arg(&mut pc, true);
                },
                OpCode::EntryProc => {
                    rem_bytes = self.read_arg(pc);
                    print!("{:0HEX_ARG_SIZE$X}, ", rem_bytes);
                    pc += ARG_SIZE;
                    print_arg(&mut pc, false);
                    print_arg(&mut pc, true);
                    print!(" <<< Procedure start");
                    procedure_count -= 1;
                }
                OpCode::PutString => {
                    let strb: Vec<_> = self.program.iter().skip(pc).take_while(|&&b| b != 0).map(|b| *b).collect();
                    pc += strb.len() + 1;
                    let str = String::from_utf8(strb).expect("Invalid UTF-8");
                    print!("\"{str}\"");
                }
                _ => {},
            }
            rem_bytes -= (pc - opc) as i16;

            println!();

            if rem_bytes <= 0 && procedure_count == 0 { break; }
        }
        (0..((self.program.len() - pc) / self.data_size())).map(|i| self.read_data(pc + self.data_size() * i)).enumerate().for_each(|(i, constant)| {
            let ds2 = self.data_size() * 2;
            let c = constant.i64();
            println!("Constant {:04}: 0x{:0ds2$X} = {}", i, c, c);
        });
    }

    fn load_data(&self) -> (Vec<Procedure>, Vec<Data>) {
        let mut procedure_count = self.read_arg(0);
        let mut procedures = Vec::with_capacity(procedure_count as usize);
        procedures.resize_with(procedures.capacity(), || None);
        let mut pc = 4;

        let mut rem_bytes = 0;
        loop {
            let byte = self.program[pc];
            let opc = pc;
            pc += 1;
            if rem_bytes == 0 && byte == OpCode::EntryProc.into() {
                rem_bytes = self.read_arg(pc);
                pc += ARG_SIZE;
                let proc_id = self.read_arg(pc) as usize;
                pc += ARG_SIZE * 2;
                procedures[proc_id] = Some(Procedure {
                    start_pos: pc - 1 - ARG_SIZE * 3,
                    frame_ptr: 0,
                });
                procedure_count -= 1;
            }
            rem_bytes -= (pc - opc) as i16;

            if rem_bytes <= 0 && procedure_count == 0 { break; }
        }
        (
            procedures.into_iter().map(|procedure| procedure.unwrap()).collect(),
            (0..((self.program.len() - pc) / self.data_size())).map(|i| self.read_data(pc + self.data_size() * i)).collect(),
        )
    }

    //noinspection RsConstantConditionIf
    pub fn execute(&self) {
        let (procedures, constants) = self.load_data();

        procedures.iter().for_each(|procedure| println!("{:?}", procedure));
        constants.iter().enumerate().for_each(|(i, constant)| println!("const {i}: {:?}", constant.i64()));

        let mut pc = procedures[0].start_pos;
        let mut stack: Vec<u8> = vec![];
        let mut fp = 0usize;

        let pop_data = |stack: &mut Vec<u8>| -> Data {
            self.bytes_to_data(stack.drain(stack.len() - self.data_size()..).as_ref())
        };
        let push_data = |stack: &mut Vec<u8>, data: Data| {
            stack.append(&mut data.to_bytes());
        };
        let read_arg = |pc: &mut usize| -> i16 {
            *pc += ARG_SIZE;
            self.read_arg(*pc - ARG_SIZE)
        };
        let set_addr = |fp: &usize, stack: &mut Vec<u8>, data: &Data| {
            if stack.len() < (fp + self.data_size()) { stack.resize(fp + self.data_size(), 0); }
            stack.splice(fp..&(fp + self.data_size()), data.i64().to_le_bytes());
        };

        loop {
            let byte = self.program[pc];

            let op = OpCode::try_from(byte).unwrap_or(OpCode::EndOfCode);
            if DEBUG { print!("{:?}", op); }
            pc += 1;
            match op {
                OpCode::EntryProc => {
                    pc += ARG_SIZE * 3;
                },
                OpCode::ReturnProc => {
                    break;
                },
                OpCode::PushValueLocalVar => {}
                OpCode::PushValueMainVar => {}
                OpCode::PushValueGlobalVar => {}
                OpCode::PushAddressLocalVar => {}
                OpCode::PushAddressMainVar => {}
                OpCode::PushAddressGlobalVar => {}
                OpCode::PushConstant => {
                    let c = read_arg(&mut pc);
                    let cd = constants[c as usize].clone();
                    if DEBUG { print!(" at {c} => {}", cd.i64()); }
                    push_data(&mut stack, cd);
                }
                OpCode::StoreValue => {}
                OpCode::OutputValue => {
                    let data = pop_data(&mut stack);
                    if DEBUG { print!(": {}", data.i64()); }
                    println!("{}", data.i64());
                }
                OpCode::InputToAddr => {
                    let addr = pop_data(&mut stack);
                    if DEBUG { print!(" to {}", addr.i64()); }
                    let mut line = String::new();
                    stdin().lock().read_line(&mut line).expect("Input failed");
                    let input: i64 = line.trim().parse().expect("number input required");
                    set_addr(&fp, &mut stack, &self.bytes_to_data(&input.to_le_bytes()));
                }
                OpCode::Minusify => {
                    let int = pop_data(&mut stack);
                    let data = match int {
                        B16(x) => B16(-x), B32(x) => B32(-x), B64(x) => B64(-x),
                    };
                    if DEBUG { println!(" {} => {}", int.i64(), data.i64()); }
                    push_data(&mut stack, data);
                }
                OpCode::IsOdd => {}
                OpCode::OpAdd => {
                    let val = pop_data(&mut stack).i64() + pop_data(&mut stack).i64();
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }
                OpCode::OpSubtract => {
                    let val = pop_data(&mut stack).i64() - pop_data(&mut stack).i64();
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }
                OpCode::OpMultiply => {
                    let val = pop_data(&mut stack).i64() * pop_data(&mut stack).i64();
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }
                OpCode::OpDivide => {
                    let val = pop_data(&mut stack).i64() / pop_data(&mut stack).i64();
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }
                OpCode::CompareEq => {
                    let val = pop_data(&mut stack).i64() * pop_data(&mut stack).i64();
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }
                OpCode::CompareNotEq => {}
                OpCode::CompareLT => {}
                OpCode::CompareGT => {}
                OpCode::CompareLTEq => {}
                OpCode::CompareGTEq => {}
                OpCode::CallProc => {}
                OpCode::Jump => {}
                OpCode::JumpIfFalse => {}
                OpCode::PutString => {}
                OpCode::Pop => {}
                OpCode::Swap => {}
                OpCode::EndOfCode => {}
                OpCode::Put => {}
                OpCode::Get => {}
                OpCode::OpAddAddr => {}
            }

            if DEBUG { println!(); }
        }
    }
}
