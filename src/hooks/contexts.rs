use super::registers::*;

/// The state of the general purpose registers.
/// 
/// This context is provided by an inline hook, which can occur on any instruction.
/// The inline hook will backup the general purpose registers into this context
/// and provide it by reference to the callback. After the callback, the register
/// contents are restored from this context, meaning they can be modified.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct InlineCtx {
    /// The 31 general purpose registers on an Aarch64 system (x0-x30)
    pub registers: [CpuRegister; 31]
}   

/// A more complete system context than [`InlineCtx`].
/// 
/// Due to the larger stack size requirement (3 times as much stack), this extended
/// context is only provided by an ex inline hook, which is not the default.
/// 
/// As with the [`InlineCtx`], this is provided by the hook to the callback, and
/// its contents are restored after the callback (with the exception of the stack pointer).
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ExInlineCtx {
    /// The 31 general purpose registers on an Aarch64 system (x0-x30)
    pub registers: [CpuRegister; 31],

    /// The stack pointer, this is not restored by the hooking environment,
    /// meaning it is effectively read-only
    pub sp: CpuRegister,

    /// The NEON/SIMD registers
    pub fpu_registers: [FpuRegister; 32]
}