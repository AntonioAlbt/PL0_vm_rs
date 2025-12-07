use crate::pl0_vm::PL0VM;

mod pl0_vm;
mod opcodes;

fn main() {
    let pl0vm = PL0VM::from_file("small.cl0").expect("Failed loading PL0VM");
    pl0vm.debug_print();

    println!();

    pl0vm.execute();
}
