use std::fmt::{Display};
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum OpCode {
    PushValueLocalVar = 0x0, // (i16 offset)
    PushValueMainVar = 0x1,  // (i16 offset)
    PushValueGlobalVar = 0x2, // (i16 offset, i16 procedure)
    PushAddressLocalVar = 0x3,  // (i16 offset)
    PushAddressMainVar = 0x4, // (i16 offset)
    PushAddressGlobalVar = 0x5, // (i16 offset, i16 procedure)
    PushConstant = 0x6, // (i16 index)
    StoreValue = 0x7, // erwartet auf Stack: Wert, dann Zieladresse
    OutputValue = 0x8, // println
    InputToAddr = 0x9, // readln

    // Operatoren mit 1 Faktor
    Minusify = 0xA,
    IsOdd = 0xB,

    // Operatoren mit 2 Faktoren
    OpAdd = 0xC,
    OpSubtract = 0xD,
    OpMultiply = 0xE,
    OpDivide = 0xF,

    // Boolean-Logik
    CompareEq = 0x10,
    CompareNotEq = 0x11,
    CompareLT = 0x12,
    CompareGT = 0x13,
    CompareLTEq = 0x14,
    CompareGTEq = 0x15,

    // Flow-Logik
    CallProc = 0x16,
    ReturnProc = 0x17,
    Jump = 0x18,
    JumpIfFalse = 0x19,
    EntryProc = 0x1A,

    // erweiterte Codes
    PutString = 0x1B,
    Pop = 0x1C,
    Swap = 0x1D,

    // nur für VM
    EndOfCode = 0x1E,

    // neue Codes - Funktionsweise unbekannt
    Put = 0x1F,
    Get = 0x20,
    OpAddAddr = 0x21
}

impl Display for OpCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(&format!("{:?}", self))
    }
}
