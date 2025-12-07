use crate::opcodes::OpCode;
use crate::pl0_vm::Data::{B16, B32, B64};
use std::fmt::Debug;
use std::io::{stderr, stdin, BufRead, Write};

fn error(msg: &str) {
    stderr().write(msg.as_bytes()).expect("Could not write to stderr");
    stderr().write("\n".as_bytes()).expect("Could not write to stderr");
}

const ARG_SIZE: usize = 2;
const HEX_ARG_SIZE: usize = ARG_SIZE * 2;

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
        self.clone().into()
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
    debug: bool,
}

impl PL0VM {
    pub fn new(debug: bool) -> PL0VM {
        PL0VM {
            program: vec![],
            bits: B16(0),
            debug,
        }
    }
    fn data_size(&self) -> usize { match self.bits { B16(_) => 2, B32(_) => 4, B64(_) => 8 } }

    fn data_true(&self) -> Data { match self.bits { B16(_) => B16(1), B32(_) => B32(1), B64(_) => B64(1) } }
    fn data_false(&self) -> Data { match self.bits { B16(_) => B16(0), B32(_) => B32(0), B64(_) => B64(0) } }
    fn data_bool(&self, val: bool) -> Data { match val { true => self.data_true(), false => self.data_false() } }

    pub fn from_file(debug: bool, filename: &str) -> Result<PL0VM, std::io::Error> {
        let mut pl0vm = PL0VM::new(debug);
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

    pub fn print_analysis(&self) {
        let mut pc = 4;
        let mut procedure_count = self.read_arg(0);
        print!("0000: Procedure count: {:04X} = {}, ", procedure_count, procedure_count);
        let arch = self.read_arg(ARG_SIZE);
        print!("Architecture: {:04X} = ", arch);
        match arch {
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
            let op = match OpCode::try_from(byte) {
                Ok(op) => op,
                Err(_) => {
                    error(&format!("unknown opcode: 0x{:02X}", byte));
                    break;
                },
            };
            print!("{:04X}: {:02X} {:<21} ", pc, byte, op);
            pc += 1;
            match op {
                OpCode::PushValueLocalVar | OpCode::PushValueMainVar
                    | OpCode::PushAddressLocalVar | OpCode::PushAddressMainVar
                    | OpCode::CallProc | OpCode::PushConstant => {
                    print_arg(&mut pc, true);
                },
                OpCode::Jump | OpCode::JumpIfFalse => {
                    let arg = self.read_arg(pc);
                    let target = match (pc + ARG_SIZE).checked_add_signed(arg as isize) {
                        Some(target) => target,
                        None => {
                            error(&format!("invalid jump target: from {pc} jumping {arg}"));
                            break;
                        },
                    };
                    print!("{}{:0HEX_ARG_SIZE$X} => {:0HEX_ARG_SIZE$X}", if arg < 0 { "-" } else { "" }, arg.abs(), target);
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
                    let pid = self.read_arg(pc);
                    print!("{:0HEX_ARG_SIZE$X}, ", pid);
                    pc += ARG_SIZE;
                    print_arg(&mut pc, true);
                    print!(" <<< Procedure start{}", if pid == 0 { " - main" } else { "" });
                    procedure_count -= 1;
                }
                OpCode::PutString => {
                    let strb: Vec<_> = self.program.iter().skip(pc).take_while(|&&b| b != 0).map(|b| *b).collect();
                    pc += strb.len() + 1;
                    let str = match String::from_utf8(strb) {
                        Ok(str) => str,
                        Err(err) => {
                            error(&format!("invalid string contents: {}", err));
                            break;
                        }
                    };
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
            let cstr = format!("{:0ds2$X}", c);
            println!("Constant {:04}: 0x{} = {}", i, &cstr[cstr.len() - ds2..], c);
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
        let (mut procedures, constants) = self.load_data();

        // procedures.iter().for_each(|procedure| println!("{:?}", procedure));
        // constants.iter().enumerate().for_each(|(i, constant)| println!("const {i}: {:?}", constant.i64()));

        let mut pc = procedures[0].start_pos;
        let mut stack: Vec<u8> = vec![];
        let mut fp = 0usize;
        let mut cur_proc_i = 0usize;

        let pop_data = |stack: &mut Vec<u8>| -> Data {
            self.bytes_to_data(stack.drain(stack.len() - self.data_size()..).as_ref())
        };
        let push_data = |stack: &mut Vec<u8>, data: Data| {
            stack.append(&mut data.to_bytes());
        };
        let pop_argument = |pc: &mut usize| -> i16 {
            *pc += ARG_SIZE;
            self.read_arg(*pc - ARG_SIZE)
        };
        let set_addr = |stack: &mut Vec<u8>, fp: &usize, data: &Data| {
            if stack.len() < (fp + self.data_size()) { stack.resize(fp + self.data_size(), 0); }
            let bytes = match data {
                B16(v) => v.to_le_bytes().to_vec(), B32(v) => v.to_le_bytes().to_vec(), B64(v) => v.to_le_bytes().to_vec(),
            };
            stack.splice(fp..&(fp + self.data_size()), bytes);
        };
        let offsetted = |start: &usize, offset: isize| start.checked_add_signed(offset).expect("invalid variable offset");

        let arch_bytes = self.read_arg(0);
        if self.debug {
            println!("\t@0000: {:<21}{arch_bytes:04X} = {}", "Set Architecture", match arch_bytes {
                2 => "16 bit",
                4 => "32 bit",
                8 => "64 bit",
                _ => "invalid",
            });
        }
        if arch_bytes != 2 && arch_bytes != 4 && arch_bytes != 8 {
            error(&format!("invalid architecture bytes: {arch_bytes:04X} (allowed: 2, 4, 8)"));
            return;
        }

        loop {
            let byte = self.program[pc];

            let op = match OpCode::try_from(byte) {
                Ok(op) => op,
                Err(_) => {
                    error(&format!("unknown opcode: 0x{:02X}", byte));
                    break;
                },
            };
            if self.debug { print!("\t@{pc:04X}: {:<21}", op); }
            pc += 1;
            match op {
                OpCode::EntryProc => {
                    pc += ARG_SIZE;
                    let proc_i = pop_argument(&mut pc);
                    let varlen = pop_argument(&mut pc) as usize;
                    fp = procedures[proc_i as usize].frame_ptr;
                    stack.resize(fp + varlen, 0);
                    if self.debug { print!("reserved {varlen} bytes for variables"); }
                }
                OpCode::ReturnProc => {
                    if cur_proc_i == 0 {
                        if self.debug { println!("exiting"); }
                        break;
                    } else {
                        stack.truncate(procedures[cur_proc_i].frame_ptr);
                        let new_proc_i = u64::from_le_bytes(stack.drain(stack.len() - 8..).collect::<Vec<u8>>().try_into().expect("jumping back failed - stack invalid"));
                        let new_fp = u64::from_le_bytes(stack.drain(stack.len() - 8..).collect::<Vec<u8>>().try_into().expect("jumping back failed - stack invalid"));
                        let new_pc = u64::from_le_bytes(stack.drain(stack.len() - 8..).collect::<Vec<u8>>().try_into().expect("jumping back failed - stack invalid"));
                        if self.debug { print!("pc: {pc} => {new_pc}, fp: {fp} => {new_fp}, cpi: {cur_proc_i} => {new_proc_i}"); }
                        pc = new_pc as usize;
                        fp = new_fp as usize;
                        cur_proc_i = new_proc_i as usize;
                    }
                }
                OpCode::CallProc => {
                    let proc_id = pop_argument(&mut pc);
                    stack.extend((pc as u64).to_le_bytes());
                    stack.extend((fp as u64).to_le_bytes());
                    stack.extend((cur_proc_i as u64).to_le_bytes());
                    let proc = &mut procedures[proc_id as usize];
                    if self.debug { print!("pc: {pc} => {}, fp: {fp} => {}, cpi: {cur_proc_i} => {}", proc.start_pos, stack.len(), proc_id); }
                    cur_proc_i = proc_id as usize;
                    pc = proc.start_pos;
                    proc.frame_ptr = stack.len();
                }

                OpCode::PushValueLocalVar => {
                    let addr = pop_argument(&mut pc);
                    let data = self.bytes_to_data(&stack[offsetted(&fp, addr as isize)..]);
                    if self.debug { print!("took {} from address {}", data.i64(), offsetted(&fp, addr as isize)); }
                    push_data(&mut stack, data);
                }
                OpCode::PushValueMainVar => {
                    let addr = pop_argument(&mut pc);
                    let data = self.bytes_to_data(&stack[offsetted(&procedures[0].frame_ptr, addr as isize)..]);
                    if self.debug { print!("took {} from address {}", data.i64(), offsetted(&procedures[0].frame_ptr, addr as isize)); }
                    push_data(&mut stack, data);
                }
                OpCode::PushValueGlobalVar => {
                    let proc_index = pop_argument(&mut pc) as usize;
                    let addr = pop_argument(&mut pc);
                    let data = self.bytes_to_data(&stack[offsetted(&procedures[proc_index].frame_ptr, addr as isize)..]);
                    if self.debug { print!("took {} from address {}", data.i64(), offsetted(&procedures[proc_index].frame_ptr, addr as isize)); }
                    push_data(&mut stack, data);
                }
                OpCode::PushAddressLocalVar => {
                    let addr = pop_argument(&mut pc);
                    let data = self.bytes_to_data(&offsetted(&fp, addr as isize).to_le_bytes());
                    if self.debug { print!("pushed address {}", offsetted(&fp, addr as isize)); }
                    push_data(&mut stack, data);
                }
                OpCode::PushAddressMainVar => {
                    let addr = pop_argument(&mut pc);
                    let data = self.bytes_to_data(&offsetted(&procedures[0].frame_ptr, addr as isize).to_le_bytes());
                    if self.debug { print!("pushed address {}", offsetted(&procedures[0].frame_ptr, addr as isize)); }
                    push_data(&mut stack, data);
                }
                OpCode::PushAddressGlobalVar => {
                    let addr = pop_argument(&mut pc);
                    let proc_index = pop_argument(&mut pc) as usize;
                    if self.debug {
                        print!("from procedure {} take address {addr}", proc_index);
                        print!(" => pushed address {}", offsetted(&procedures[proc_index].frame_ptr, addr as isize));
                    }
                    let data = self.bytes_to_data(&offsetted(&procedures[proc_index].frame_ptr, addr as isize).to_le_bytes());
                    push_data(&mut stack, data);
                }
                OpCode::PushConstant => {
                    let c = pop_argument(&mut pc);
                    let cd = constants[c as usize].clone();
                    if self.debug { print!("constant {c} => pushing {}", cd.i64()); }
                    push_data(&mut stack, cd);
                }
                OpCode::StoreValue => {
                    let data = pop_data(&mut stack);
                    let addr = pop_data(&mut stack).i64();
                    if self.debug { print!("value {} at address {}", data.i64(), addr) }
                    set_addr(&mut stack, &(addr as usize), &data);
                }

                OpCode::OutputValue => {
                    let data = pop_data(&mut stack);
                    if self.debug {
                        print!("{}\n{}", data.i64(), data.i64());
                    } else {
                        println!("{}", data.i64());
                    }
                }
                OpCode::InputToAddr => {
                    let addr = pop_data(&mut stack);
                    if self.debug { println!("to address {}", addr.i64()); }
                    'input_loop: loop {
                        let mut line = String::new();
                        stdin().lock().read_line(&mut line).expect("Input failed");
                        let input: Result<i64, _> = line.trim().parse();
                        match input {
                            Ok(num) => {
                                set_addr(&mut stack, &offsetted(&fp, addr.i64() as isize), &self.bytes_to_data(&num.to_le_bytes()));
                                break 'input_loop;
                            },
                            Err(_) => {
                                error("invalid number input");
                            }
                        }
                    }
                }

                OpCode::Minusify => {
                    let int = pop_data(&mut stack);
                    let data = match int {
                        B16(x) => B16(-x), B32(x) => B32(-x), B64(x) => B64(-x),
                    };
                    if self.debug { print!("{} => {}", int.i64(), data.i64()); }
                    push_data(&mut stack, data);
                }
                OpCode::IsOdd => {
                    let int = pop_data(&mut stack).i64();
                    let val = int % 2 == 1;
                    if self.debug { print!("{} => {}", int, val); }
                    push_data(&mut stack, self.data_bool(val));
                }

                OpCode::OpAdd => {
                    let right = pop_data(&mut stack).i64();
                    let left = pop_data(&mut stack).i64();
                    let val = left + right;
                    if self.debug { print!("{left} + {right} = {val}") }
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }
                OpCode::OpSubtract => {
                    let right = pop_data(&mut stack).i64();
                    let left = pop_data(&mut stack).i64();
                    let val = left - right;
                    if self.debug { print!("{left} - {right} = {val}") }
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }
                OpCode::OpMultiply => {
                    let right = pop_data(&mut stack).i64();
                    let left = pop_data(&mut stack).i64();
                    let val = left * right;
                    if self.debug { print!("{left} * {right} = {val}") }
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }
                OpCode::OpDivide => {
                    let right = pop_data(&mut stack).i64();
                    let left = pop_data(&mut stack).i64();
                    let val = left / right;
                    if self.debug { print!("{left} / {right} = {val}") }
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }

                OpCode::CompareEq => {
                    let right = pop_data(&mut stack).i64();
                    let left = pop_data(&mut stack).i64();
                    let val = left == right;
                    if self.debug { print!("{left} == {right} = {val}") }
                    push_data(&mut stack, self.data_bool(val));
                }
                OpCode::CompareNotEq => {
                    let right = pop_data(&mut stack).i64();
                    let left = pop_data(&mut stack).i64();
                    let val = left != right;
                    if self.debug { print!("{left} != {right} = {val}") }
                    push_data(&mut stack, self.data_bool(val));
                }
                OpCode::CompareLT => {
                    let right = pop_data(&mut stack).i64();
                    let left = pop_data(&mut stack).i64();
                    let val = left < right;
                    if self.debug { print!("{left} < {right} = {val}") }
                    push_data(&mut stack, self.data_bool(val));
                }
                OpCode::CompareGT => {
                    let right = pop_data(&mut stack).i64();
                    let left = pop_data(&mut stack).i64();
                    let val = left > right;
                    if self.debug { print!("{left} > {right} = {val}") }
                    push_data(&mut stack, self.data_bool(val));
                }
                OpCode::CompareLTEq => {
                    let right = pop_data(&mut stack).i64();
                    let left = pop_data(&mut stack).i64();
                    let val = left <= right;
                    if self.debug { print!("{left} <= {right} = {val}") }
                    push_data(&mut stack, self.data_bool(val));
                }
                OpCode::CompareGTEq => {
                    let right = pop_data(&mut stack).i64();
                    let left = pop_data(&mut stack).i64();
                    let val = left >= right;
                    if self.debug { print!("{left} >= {right} = {val}") }
                    push_data(&mut stack, self.data_bool(val));
                }

                OpCode::Jump => {
                    let offset = pop_argument(&mut pc);
                    pc = offsetted(&pc, offset as isize);
                    if self.debug { print!("jumping to {pc}"); }
                }
                OpCode::JumpIfFalse => {
                    let dat = pop_data(&mut stack).i64();
                    let offset = pop_argument(&mut pc);
                    if self.debug { print!("jumping: {}", dat == 0); }
                    if dat == 0 {
                        pc = offsetted(&pc, offset as isize);
                        if self.debug { print!(" to {pc:04X}"); }
                    }
                }

                OpCode::PutString => {
                    let bytes: Vec<u8> = self.program[pc..].iter().take_while(|&&b| b != 0).map(|&b| b).collect();
                    pc += bytes.len() + 1;
                    let str = match String::from_utf8(bytes) {
                        Ok(str) => str,
                        Err(err) => {
                            error(&format!("\ninvalid string contents: {}", err));
                            break;
                        }
                    };
                    if self.debug {
                        print!("\"{str}\"\n{str}");
                    } else {
                        println!("{str}");
                    }
                }

                OpCode::Pop => {
                    if self.debug {
                        println!("popped {}", pop_data(&mut stack).i64());
                    } else {
                        pop_data(&mut stack);
                    }
                }
                OpCode::Swap => {
                    let offset = pop_data(&mut stack).i64();
                    let data = self.bytes_to_data(&stack[(offset as usize)..]);
                    if self.debug { print!("address {} => data {}", offset as usize, data.i64()) }
                    push_data(&mut stack, data);
                }

                OpCode::EndOfCode => {
                    if self.debug { println!(); }
                    break;
                }

                OpCode::Put => { todo!() }
                OpCode::Get => { todo!() }
                OpCode::OpAddAddr => { todo!() }
            }

            match op {
                OpCode::InputToAddr => (),
                _ => if self.debug { println!(); }
            };
        }
    }
}
