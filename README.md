# pl0_vm_rs

VM for bytecode of HTW Dresden Compilerbau PL0 compiler.

Usage: `pl0_vm_rs [flags] <filename>`

See more information with `pl0_vm_rs --help`.

## Benchmarks

`cargo bench` runs the release binary on the programs in `benches/programs/`
(PL0 sources + compiled `.cl0` files) to benchmark the VM and checks their output for correctness as well.

Compare VM versions:

```sh
cargo bench -- --save-baseline before   # on the old version
# now change something
cargo bench -- --baseline before        # shows the change per program
```

You can also benchmark other VMs which run CL0:

```sh
PL0_VM=/path/to/other-vm cargo bench -- --save-baseline other-vm
# then compare
cargo bench -- --baseline other-vm
```
