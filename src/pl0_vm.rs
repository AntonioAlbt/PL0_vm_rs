use crate::opcodes::OpCode;
use std::fmt::Debug;
use std::io::{BufRead, Write, stderr, stdin};
use std::marker::PhantomData;
use rust_i18n::t;

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

trait Word: Copy + Into<i64> + std::fmt::Display {
    const SIZE: usize;
    const TRUE: Self;
    const FALSE: Self;

    fn bool(val: bool) -> Self;

    fn from_bytes(data: &[u8]) -> Self;
    fn try_from_bytes(data: Option<&[u8]>) -> Option<Self>;
    fn from_i64(val: i64) -> Self;
    fn pop(stack: &mut Vec<u8>) -> Option<Self>;

    fn i64(self) -> i64;
    fn push(self, stack: &mut Vec<u8>);
    fn write(self, stack: &mut Vec<u8>, at: usize);
    fn invert(self) -> Self;
}

macro_rules! impl_word {
    ($t:ty, $size:expr) => {
        impl Word for $t {
            const SIZE: usize = $size;
            const TRUE: Self = 1;
            const FALSE: Self = 0;

            fn from_bytes(data: &[u8]) -> $t {
                <$t>::from_le_bytes(data[0..$size].try_into().expect("Invalid byte count?!"))
            }

            fn try_from_bytes(data: Option<&[u8]>) -> Option<$t> {
                match data {
                    Some(d) => Some(<$t>::from_le_bytes(d[0..$size].try_into().ok()?)),
                    None => None,
                }
            }

            fn pop(stack: &mut Vec<u8>) -> Option<Self> {
                let len = stack.len();

                if len < Self::SIZE {
                    return None;
                }

                let start = len - Self::SIZE;
                
                let data_bytes = &stack[start..(start + Self::SIZE)];
                let data = Self::from_bytes(data_bytes);

                stack.truncate(start);

                Some(data)
            }

            fn bool(val: bool) -> Self {
                if val {
                    Self::TRUE
                } else {
                    Self::FALSE
                }
            }

            fn from_i64(val: i64) -> Self {
                val as $t
            }



            fn i64(self) -> i64 {
                self.into()
            }

            fn push(self, stack: &mut Vec<u8>) {
                stack.extend(self.to_le_bytes());
            }

            fn write(self, stack: &mut Vec<u8>, at: usize) {
                if stack.len() < (at + Self::SIZE) { stack.resize(at + Self::SIZE, 0); }
                stack[at..(at + Self::SIZE)].copy_from_slice(&self.to_le_bytes());
            }

            fn invert(self) -> Self {
                -self
            }
        }
    };
}

impl_word!(i16, 2);
impl_word!(i32, 4);
impl_word!(i64, 8);

pub trait Vm {
    fn execute(&self);
    fn print_analysis(&self);
}

impl<W: Word, const DEBUG: bool> Vm for PL0VM<W, DEBUG> {
    fn execute(&self) { PL0VM::execute(self) }
    fn print_analysis(&self) { PL0VM::print_analysis(self) }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("could not read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("file is too short to contain a header")]
    MissingHeader,
    #[error("unsupported word size {0} (expected 2, 4 or 8)")]
    UnsupportedWordSize(i16),
}

pub fn load_vm_from_file(filename: &str, debug: bool) -> Result<Box<dyn Vm>, LoadError> {
    let bytes = std::fs::read(filename)?;          // `?` converts the io::Error via From
    // turn the runtime bool into a const generic once, here:
    if debug { from_bytes::<true>(bytes) } else { from_bytes::<false>(bytes) }
}

fn from_bytes<const DEBUG: bool>(bytes: Vec<u8>) -> Result<Box<dyn Vm>, LoadError> {
    let word_size = match bytes.get(ARG_SIZE..ARG_SIZE*2) {
        Some(&[lo, hi]) => i16::from_le_bytes([lo, hi]),   // slice pattern, no try_into/expect
        _ => return Err(LoadError::MissingHeader),
    };

    // the type annotation lets each arm coerce to Box<dyn Vm>
    let vm: Box<dyn Vm> = match word_size {
        2 => Box::new(PL0VM::<i16, DEBUG>::new(bytes)),
        4 => Box::new(PL0VM::<i32, DEBUG>::new(bytes)),
        8 => Box::new(PL0VM::<i64, DEBUG>::new(bytes)),
        n => return Err(LoadError::UnsupportedWordSize(n)),
    };
    Ok(vm)
}

struct PL0VM<W: Word, const DEBUG: bool> {
    program: Vec<u8>,
    x: PhantomData<W>
}

impl<Data: Word, const DEBUG: bool> PL0VM<Data, DEBUG> {
    pub fn new(program: Vec<u8>) -> PL0VM<Data, DEBUG> {
        PL0VM::<Data, DEBUG> {
            program: program,
            x: PhantomData,
        }
    }

    fn read_arg(&self, offset: usize) -> Option<i16> {
        match self.program.get(offset..(offset + ARG_SIZE)) {
            Some(val) => Some(i16::from_le_bytes(val.try_into().expect("Invalid byte count?!"))),
            None => None
        }
    }

    fn read_data(&self, offset: usize) -> Option<Data> {
        Data::try_from_bytes(self.program.get(offset..))
    }

    pub fn print_analysis(&self) {
        if self.program.len() <= 4 || self.program[3] > 0 {
            error(&t!("pl0.invalid_file"));
            return;
        }

        let mut pc = 4;
        let mut procedure_count = match self.read_arg(0) {
            Some(val) => val,
            None => return error("unreachable code"),
        };
        print!("0000: {}: {:04X} = {}, ", t!("pl0.procedure_count"), procedure_count, procedure_count);
        let arch = match self.read_arg(ARG_SIZE) {
            Some(val) => val,
            None => return error("unreachable code"),
        };
        print!("{}: {:04X} = ", t!("pl0.arch"), arch);
        match arch {
            2 => println!("16 bit"),
            4 => println!("32 bit"),
            8 => println!("64 bit"),
            _ => println!("{}", t!("pl0.invalid")),
        }
        if arch != 2 && arch != 4 && arch != 8 {
            error(&t!("pl0.arch_invalid", arch = arch:{:04X}));
            return;
        }

        let print_arg = |pc: &mut usize, last: bool| {
            let val = match self.read_arg(*pc) {
                Some(val) => val,
                None => return false,
            };
            print!("{:0HEX_ARG_SIZE$X}{}", val, if last { "" } else { ", " });
            *pc += ARG_SIZE;
            return true;
        };

        let mut rem_bytes = 0;
        loop {
            let byte = match self.program.get(pc) {
                Some(val) => val,
                None => return error(&t!("pl0.error.invalid_pc", pc = pc:{:04X})),
            };
            let opc = pc;
            let op = match OpCode::try_from(*byte) {
                Ok(op) => op,
                Err(_) => {
                    error(&t!("pl0.unknown_opcode", op = byte:{:02X}));
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
                    let arg = match self.read_arg(pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    let target = match (pc + ARG_SIZE).checked_add_signed(arg as isize) {
                        Some(target) => target,
                        None => {
                            error(&t!("pl0.invalid_jump", pc = pc, arg = arg));
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
                    rem_bytes = match self.read_arg(pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    print!("{:0HEX_ARG_SIZE$X}, ", rem_bytes);
                    pc += ARG_SIZE;
                    let pid = match self.read_arg(pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    print!("{:0HEX_ARG_SIZE$X}, ", pid);
                    pc += ARG_SIZE;
                    print_arg(&mut pc, true);
                    print!(" <<< {}", if pid == 0 { t!("pl0.proc_start_main") } else { t!("pl0.proc_start") });
                    procedure_count -= 1;
                }
                OpCode::PutString => {
                    let strb: Vec<_> = self.program.iter().skip(pc).take_while(|&&b| b != 0).map(|b| *b).collect();
                    pc += strb.len() + 1;
                    let str = match String::from_utf8(strb) {
                        Ok(str) => str,
                        Err(err) => {
                            error(&t!("pl0.invalid_str", err = err));
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
        (0..((self.program.len() - pc) / Data::SIZE)).map(|i| self.read_data(pc + Data::SIZE * i)).enumerate().for_each(|(i, constant)| {
            let ds2 = Data::SIZE * 2;
            let c = match constant {
                Some(val) => val,
                None => return error(&t!("pl0.error.invalid_constant_read", i = i)),
            }.i64();
            let cstr = format!("{:0ds2$X}", c);
            println!("{} {:04}: 0x{} = {}", t!("pl0.constant"), i, &cstr[cstr.len() - ds2..], c);
        });
    }

    fn load_data(&self) -> Option<(Vec<Procedure>, Vec<Data>)> {
        let mut procedure_count = self.read_arg(0).expect("failed to read procedure count - should be unreachable");
        let mut procedures = Vec::with_capacity(procedure_count as usize);
        procedures.resize_with(procedures.capacity(), || None);
        let mut pc = 4;

        let mut rem_bytes = 0;
        loop {
            let byte = match self.program.get(pc) {
                Some(val) => *val,
                None => { error(&t!("pl0.error.preload_error")); return None },
            };
            let opc = pc;
            pc += 1;
            if rem_bytes == 0 && byte == <OpCode as Into<u8>>::into(OpCode::EntryProc) {
                rem_bytes = match self.read_arg(pc) {
                    Some(val) => val,
                    None => { error(&t!("pl0.error.preload_error")); return None },
                };
                pc += ARG_SIZE;
                let proc_id = match self.read_arg(pc) {
                    Some(val) => val,
                    None => { error(&t!("pl0.error.preload_error")); return None },
                } as usize;
                pc += ARG_SIZE * 2;
                if proc_id >= procedures.len() {
                    error(&t!("pl0.error.invalid_preload_procedure"));
                    return None;
                }
                procedures[proc_id] = Some(Procedure {
                    start_pos: pc - 1 - ARG_SIZE * 3,
                    frame_ptr: 0,
                });
                procedure_count -= 1;
            }
            rem_bytes -= (pc - opc) as i16;

            if rem_bytes <= 0 && procedure_count == 0 { break; }
        }
        Some((
            procedures.into_iter().map(|procedure| procedure.unwrap()).collect(),
            (0..((self.program.len() - pc) / Data::SIZE)).map(|i| self.read_data(pc + Data::SIZE * i).expect(&t!("pl0.error.invalid_constant_read", i = i))).collect(),
        ))
    }

    //noinspection RsConstantConditionIf
    pub fn execute(&self) {
        if self.program.len() <= 4 || self.program[3] > 0 {
            error(&t!("pl0.invalid_file"));
            return;
        }

        // --- architecture check ---
        let arch_bytes = match self.read_arg(ARG_SIZE) {
            Some(val) => val,
            None => return error(&t!("pl0.error.failed_arch_read")),
        };
        if DEBUG {
            let invalid = t!("pl0.invalid");
            println!("\t@0000: {:<21}{arch_bytes:04X} = {}", t!("pl0.set_arch"), match arch_bytes {
                2 => "16 bit",
                4 => "32 bit",
                8 => "64 bit",
                _ => &invalid,
            });
        }
        if arch_bytes != 2 && arch_bytes != 4 && arch_bytes != 8 {
            error(&t!("pl0.arch_invalid", arch = arch_bytes:{:04X}));
            return;
        }

        let (mut procedures, constants) = match self.load_data() {
            Some(val) => val,
            None => return,
        };

        // --- execution state ---
        // program counter = index of currently executed byte
        let mut pc = procedures[0].start_pos;
        // stack = contains all dynamic runtime data
        let mut stack: Vec<u8> = Vec::with_capacity(4096); // reserve 4 KB at startup
        // frame pointer = index of start of current stack frame in vector stack
        let mut fp = 0usize;
        // current procedure index = index of current procedure in vector procedures
        let mut cur_proc_i = 0usize;

        // --- collection of functions used for execution ---
        // pop one argument from the bytecode, by increasing the program counter by ARG_SIZE
        let pop_argument = |pc: &mut usize| -> Option<i16> {
            *pc += ARG_SIZE;
            self.read_arg(*pc - ARG_SIZE)
        };
        // calculate the address start + offset, with respect to types
        let offsetted = |start: &usize, offset: isize| start.checked_add_signed(offset).expect("invalid variable offset");

        // --- main execution loop ---
        loop {
            let byte = self.program[pc];

            // try to get op code from current byte
            let op = match OpCode::try_from(byte) {
                Ok(op) => op,
                Err(_) => {
                    error(&t!("pl0.unknown_opcode", op = byte:{:02X}));
                    break;
                },
            };
            if DEBUG { print!("\t@{pc:04X}: {:<21}", op); }
            // increase program counter already, so that next pop_argument call returns valid data
            pc += 1;
            match op {
                OpCode::EntryProc => {
                    pc += ARG_SIZE;
                    let proc_i = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if proc_i < 0 {
                        error(&t!("pl0.enter_invalid_proc", id = proc_i));
                        return;
                    }
                    let varlen = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    } as usize;
                    fp = procedures[proc_i as usize].frame_ptr;
                    stack.resize(fp + varlen, 0);
                    if DEBUG { print!("{}", t!("pl0.reserved_varspace", bytes = varlen)); }
                }
                OpCode::ReturnProc => {
                    if cur_proc_i == 0 {
                        if DEBUG { println!("{}", t!("pl0.exiting")); }
                        break;
                    } else {
                        stack.truncate(procedures[cur_proc_i].frame_ptr);
                        // restore the frame pointer of the previous activation of this procedure (needed for recursion)
                        let base = procedures[cur_proc_i].frame_ptr;
                        let rd = |i: usize| u64::from_le_bytes(stack[i..i + 8].try_into().expect("jumping back failed - stack invalid")) as usize;
                        let old_frame_ptr = rd(base - 8);
                        let new_proc_i    = rd(base - 16);
                        let new_fp        = rd(base - 24);
                        let new_pc        = rd(base - 32);
                        stack.truncate(base - 32);
                        if DEBUG {
                            let pre_fp = procedures[cur_proc_i].frame_ptr;
                            print!("pc: {pc} => {new_pc}, fp: {fp} => {new_fp}, cpi: {cur_proc_i} => {new_proc_i}, old fp: {pre_fp} => {old_frame_ptr}");
                        }
                        pc = new_pc as usize;
                        fp = new_fp as usize;
                        procedures[cur_proc_i].frame_ptr = old_frame_ptr as usize;
                        cur_proc_i = new_proc_i as usize;
                    }
                }
                OpCode::CallProc => {
                    let proc_id = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if proc_id < 0 {
                        error(&t!("pl0.call_invalid_proc", id = proc_id));
                        return;
                    }
                    stack.extend((pc as u64).to_le_bytes());
                    stack.extend((fp as u64).to_le_bytes());
                    stack.extend((cur_proc_i as u64).to_le_bytes());
                    let proc = &mut procedures[proc_id as usize];
                    // save the callee's current frame pointer, so a recursive call does not lose the outer activation's frame
                    stack.extend((proc.frame_ptr as u64).to_le_bytes());
                    if DEBUG { print!("pc: {pc} => {}, fp: {fp} => {}, cpi: {cur_proc_i} => {}", proc.start_pos, stack.len(), proc_id); }
                    cur_proc_i = proc_id as usize;
                    pc = proc.start_pos;
                    proc.frame_ptr = stack.len();
                }

                OpCode::PushValueLocalVar => {
                    let addr = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if addr < 0 {
                        error(&t!("pl0.invalid_local_var_val", addr = addr));
                        return;
                    }
                    let data = match Data::try_from_bytes(stack.get(offsetted(&fp, addr as isize)..)) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if DEBUG { print!("{}", t!("pl0.took_from_addr", val = data.i64(), addr = offsetted(&fp, addr as isize))); }
                    data.push(&mut stack);
                }
                OpCode::PushValueMainVar => {
                    let addr = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if addr < 0 {
                        error(&t!("pl0.invalid_main_var_val", addr = addr));
                        return;
                    }
                    let data = match Data::try_from_bytes(stack.get(offsetted(&procedures[0].frame_ptr, addr as isize)..)) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if DEBUG { print!("{}", t!("pl0.took_from_addr", val = data.i64(), addr = offsetted(&procedures[0].frame_ptr, addr as isize))); }
                    data.push(&mut stack);
                }
                OpCode::PushValueGlobalVar => {
                    let addr = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    let proc_index = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    } as usize;
                    if addr < 0 {
                        error(&t!("pl0.invalid_global_var_val", addr = addr, proc_index = proc_index));
                        return;
                    }
                    let data = match Data::try_from_bytes(stack.get(offsetted(&procedures[proc_index].frame_ptr, addr as isize)..)) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if DEBUG { print!("{}", t!("pl0.took_from_addr", val = data.i64(), addr = offsetted(&procedures[proc_index].frame_ptr, addr as isize))); }
                    data.push(&mut stack);
                }
                OpCode::PushAddressLocalVar => {
                    let addr = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if addr < 0 {
                        error(&t!("pl0.invalid_local_var_addr", addr = addr));
                        return;
                    }
                    let data = Data::try_from_bytes(Some(&offsetted(&fp, addr as isize).to_le_bytes())).expect("failed to convert offset to Data");
                    if DEBUG { print!("{}", t!("pl0.pushed_addr", addr = offsetted(&fp, addr as isize))); }
                    data.push(&mut stack);
                }
                OpCode::PushAddressMainVar => {
                    let addr = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if addr < 0 {
                        error(&t!("pl0.invalid_main_var_addr", addr = addr));
                        return;
                    }
                    let data = Data::try_from_bytes(Some(&offsetted(&procedures[0].frame_ptr, addr as isize).to_le_bytes())).expect("failed to convert offset to Data");
                    if DEBUG { print!("{}", t!("pl0.pushed_addr", addr = offsetted(&procedures[0].frame_ptr, addr as isize))); }
                    data.push(&mut stack);
                }
                OpCode::PushAddressGlobalVar => {
                    let addr = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    let proc_index = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    } as usize;
                    if addr < 0 {
                        error(&t!("pl0.invalid_global_var_addr", addr = addr, proc_index = proc_index));
                        return;
                    }
                    if DEBUG {
                        print!("{}", t!("pl0.pushed_global_addr", proc_index = proc_index, addr = addr, push_addr = offsetted(&procedures[proc_index].frame_ptr, addr as isize)));
                    }
                    let data = Data::try_from_bytes(Some(&offsetted(&procedures[proc_index].frame_ptr, addr as isize).to_le_bytes())).expect("failed to convert offset to Data");
                    data.push(&mut stack);
                }
                OpCode::PushConstant => {
                    let c = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if c < 0 {
                        error(&t!("pl0.invalid_constant", c = c));
                        return;
                    }
                    let cd = constants[c as usize].clone();
                    if DEBUG { print!("{}", t!("pl0.pushed_constant", c = c, val = cd.i64())); }
                    cd.push(&mut stack);
                }
                OpCode::StoreValue => {
                    let data = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    let addr = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    if DEBUG { print!("{}", t!("pl0.stored_value", val = data.i64(), addr = addr)) }
                    data.write(&mut stack, addr as usize);
                }

                OpCode::OutputValue => {
                    let data = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if DEBUG {
                        print!("{}\n{}", data.i64(), data.i64());
                    } else {
                        println!("{}", data.i64());
                    }
                }
                OpCode::InputToAddr => {
                    let addr = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if DEBUG { println!("{}", t!("pl0.to_address", addr = addr.i64())); }
                    // wait for user to input a valid number
                    'input_loop: loop {
                        let mut line = String::new();
                        stdin().lock().read_line(&mut line).expect("Input failed");
                        let input: Result<i64, _> = line.trim().parse();
                        match input {
                            Ok(num) => {
                                Data::from_bytes(&num.to_le_bytes()).write(&mut stack, offsetted(&fp, addr.i64() as isize));
                                break 'input_loop;
                            },
                            Err(_) => {
                                error(&t!("pl0.invalid_number_input"));
                            }
                        }
                    }
                }

                OpCode::Minusify => {
                    let int = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    let data = int.invert();
                    if DEBUG { print!("{} => {}", int.i64(), data.i64()); }
                    data.push(&mut stack);
                }
                OpCode::IsOdd => {
                    let int = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    // Done this way to handle negative numbers correctly
                    // because -1 % 2 == -1 and 1 % 2 == 1
                    let not_odd = int % 2 == 0;
                    let val = !not_odd;
                    if DEBUG { print!("{} => {}", int, val); }
                    Data::bool(val).push(&mut stack);
                }

                OpCode::OpAdd => {
                    let right = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left + right;
                    if DEBUG { print!("{left} + {right} = {val}") }
                    Data::from_i64(val).push(&mut stack);
                }
                OpCode::OpSubtract => {
                    let right = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left - right;
                    if DEBUG { print!("{left} - {right} = {val}") }
                    Data::from_i64(val).push(&mut stack);
                }
                OpCode::OpMultiply => {
                    let right = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left * right;
                    if DEBUG { print!("{left} * {right} = {val}") }
                    Data::from_i64(val).push(&mut stack);
                }
                OpCode::OpDivide => {
                    let right = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left / right;
                    if DEBUG { print!("{left} / {right} = {val}") }
                    Data::from_i64(val).push(&mut stack);
                }

                OpCode::CompareEq => {
                    let right = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left == right;
                    if DEBUG { print!("{left} == {right} = {val}") }
                    Data::bool(val).push(&mut stack);
                }
                OpCode::CompareNotEq => {
                    let right = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left != right;
                    if DEBUG { print!("{left} != {right} = {val}") }
                    Data::bool(val).push(&mut stack);
                }
                OpCode::CompareLT => {
                    let right = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left < right;
                    if DEBUG { print!("{left} < {right} = {val}") }
                    Data::bool(val).push(&mut stack);
                }
                OpCode::CompareGT => {
                    let right = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left > right;
                    if DEBUG { print!("{left} > {right} = {val}") }
                    Data::bool(val).push(&mut stack);
                }
                OpCode::CompareLTEq => {
                    let right = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left <= right;
                    if DEBUG { print!("{left} <= {right} = {val}") }
                    Data::bool(val).push(&mut stack);
                }
                OpCode::CompareGTEq => {
                    let right = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left >= right;
                    if DEBUG { print!("{left} >= {right} = {val}") }
                    Data::bool(val).push(&mut stack);
                }

                OpCode::Jump => {
                    let offset = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    pc = offsetted(&pc, offset as isize);
                    if DEBUG { print!("{}", t!("pl0.jumping_to", pc = pc:{:04X})); }
                }
                OpCode::JumpIfFalse => {
                    let dat = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let offset = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if DEBUG { print!("{}", t!("pl0.jumping_if_bool", bool = dat == 0)); }
                    if dat == 0 {
                        pc = offsetted(&pc, offset as isize);
                        if DEBUG { print!("{}", t!("pl0.jumping_if_where", pc = pc:{:04X})); }
                    }
                }

                OpCode::PutString => {
                    let bytes: Vec<u8> = self.program[pc..].iter().take_while(|&&b| b != 0).map(|&b| b).collect();
                    pc += bytes.len() + 1;
                    let str = match String::from_utf8(bytes) {
                        Ok(str) => str,
                        Err(err) => {
                            error(&format!("\n{}", t!("pl0.invalid_str", err = err)));
                            break;
                        }
                    };
                    if DEBUG {
                        print!("\"{str}\"\n{str}");
                    } else {
                        println!("{str}");
                    }
                }

                OpCode::Pop => {
                    if DEBUG {
                        println!("{}", t!("pl0.popped", data = match Data::pop(&mut stack) {
                            Some(val) => val,
                            None => return error(&t!("pl0.error.invalid_stack_read")),
                        }.i64()));
                    } else {
                        Data::pop(&mut stack);
                    }
                }
                OpCode::Swap => {
                    let offset = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let data = match Data::try_from_bytes(stack.get((offset as usize)..)) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if DEBUG { print!("{}", t!("pl0.swapped", addr = offset as usize, val = data.i64())) }
                    data.push(&mut stack);
                }

                OpCode::EndOfCode => {
                    if DEBUG { println!(); }
                    break;
                }

                /*
                ---- Store variable value at dynamic calculated address ----
                Stack before (bottom -> top): ... | AddressOffset | Value
                Stack after: ... Stack[abs_addr] = Value
            
                ---- Sequence of operations: ----
                1. Pop Value from stack
                2. Pop AddressOffset from stack
                3. Calculate AbsoluteAddress = fp + AddressOffset
                4. Write Value to stack at AbsoluteAddress
                */
                OpCode::Put => {
                    let data = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    let addr = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let abs_addr = offsetted(&fp, addr as isize);
                    if DEBUG { print!("addr (rel) = {addr}, abs_addr = {abs_addr}, value = {:?}", data.i64()); }
                    data.write(&mut stack, abs_addr);
                }

                /*
                ---- Get variable value from dynamic calculated address ----
                Stack before (bottom -> top): ... | addressOffset
                Stack after: ... | Value
            
                ---- Sequence of operations: ----
                0. ADD-Instruction:  addressOffset = baseAddress + indexOffset
                1. Pop addressOffset from stack
                2. Read Value at stack[addressOffset]
                3. push value onto the stack
                */
                OpCode::Get => {
                    let addr = match Data::pop(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let data = match Data::try_from_bytes(stack.get((addr as usize)..)) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if DEBUG { print!("addr = {addr}, value = {:?}", data.i64()); }
                    data.push(&mut stack);
                }

                OpCode::OpAddAddr => { todo!() }
            }

            match op {
                OpCode::InputToAddr => (),
                _ => if DEBUG { println!(); }
            };
        }
    }
}
