use std::fmt;

use super::contexts;

use thiserror::Error;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct StackFrame {
    pub previous_frame: *mut StackFrame,
    pub return_address: u64,
}

#[derive(Error, Debug, Copy, Clone)]
pub enum BacktraceError {
    #[error("The initial frame pointer is null")]
    InitialFPNull,

    #[error("The backtrace is recursive and the frame pointer points to itself")]
    RecursiveFramePointer,

    #[error("The backtrace is longer than the provided limit")]
    BacktraceLimitReached
}

#[derive(Debug, Copy, Clone)]
pub struct BacktraceEntry {
    ptr: *mut StackFrame,
    frame: StackFrame,
}

impl BacktraceEntry {
    /// Creates a backtrace entry from a given stack frame pointer
    pub fn new(ptr: std::ptr::NonNull<StackFrame>) -> Self {
        Self {
            ptr: ptr.as_ptr(),
            frame: unsafe { *ptr.as_ptr() }
        }
    }

    /// Gets the stack pointer of the previous frame.
    pub fn get_previous_stack_pointer(&self) -> *mut u8 {
        if self.ptr.is_null() {
            std::ptr::null_mut()
        } else {
            unsafe {
                self.ptr.add(1) as *mut u8
            }
        }
    }
}

#[derive(Debug)]
pub struct Backtrace {
    current_frame: Option<BacktraceEntry>,
    current_lr: u64,
    backtrace: [Option<Result<BacktraceEntry, BacktraceError>>; 33],
}

impl Backtrace {
    fn demangle_symbol(symbol: &'static str) -> String {
        extern "C" {
            fn __cxa_demangle(mangled: *const u8, buffer: *mut u8, length: &mut usize, status: &mut i32) -> *mut u8;
            fn free(ptr: *mut u8);
            fn strlen(str: *const u8) -> i32;
        }

        unsafe {
            let mut out_length = 0usize;
            let mut out_status = 0i32;
            let out_buffer = __cxa_demangle([symbol, "\0"].concat().as_ptr(), std::ptr::null_mut(), &mut out_length, &mut out_status);
            let result = if out_status == 0 && !out_buffer.is_null() {
                let len = strlen(out_buffer);
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(out_buffer, len as usize)).to_string()
            } else {
                symbol.to_string()
            };
            if !out_buffer.is_null() {
                free(out_buffer);
            }
            result
        }
    }

    fn get_formatted_addr_(address: u64, demangle: bool) -> String {
        if let Some(object) = crate::rtld::find_module_for_address(address) {
            let module_offset = address - object.module_base as u64;
            let name = object.get_module_name().unwrap_or("unknown");
            if let Some((sym_name, start)) = object.find_symbol_for_address(address) {
                let symbol_offset = address - start;
                if demangle {
                    format!("{:016x} ({} + {:#x}) ({} + {:#x})", address, name, module_offset, Self::demangle_symbol(sym_name), symbol_offset)
                } else {
                    format!("{:016x} ({} + {:#x}) ({} + {:#x})", address, name, module_offset, sym_name, symbol_offset)
                }
            } else {
                format!("{:016x} ({} + {:#x})", address, name, module_offset)
            }
        } else {
            format!("{:016x}", address)
        }
    }

    /// Builds a new stack backtrace based on the provided frame pointer and return address
    /// 
    /// # Arguments
    /// * `current_fp` - The pointer to the current stack frame
    /// * `current_lr` - The current return address
    /// * `limit` - The maximum number of stack frames to move back through
    /// 
    /// # Returns
    /// * `Ok(Backtrace)` - A successfully created backtrace
    /// * `Err(BacktraceError)` - A failed backtrace
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn new(mut current_fp: *mut StackFrame, current_lr: u64, mut limit: usize) -> Result<Self, BacktraceError> {
        // if the frame pointer is null then we can't really generate a stack trace any more meaningful
        // than the provided lr, which the caller should already have
        if current_fp.is_null() {
            return Err(BacktraceError::InitialFPNull);
        }

        limit = limit.max(32);

        unsafe {
            let current_frame = *current_fp;
            // If the current stack frame's LR is not the same as what
            // was provided, we can assume that the backtrace is being generated in 
            // one of three contexts:
            // 1. The surrounding function does not make use of the frame pointer and does not
            //      push it, which usually means that they aren't calling any other functions
            //      You can see an example of this here: https://godbolt.org/z/Weza98z3q
            //      Here, `main` pushes the frame pointer, calls `something` which pushes the frame pointer
            //      which then calls `something2`, which uses the stack but doesn't push the frame pointer
            //      since it doesn't need to worry about any internal function calls messing up
            //      the x30 register (which is used as the return address)
            // 2. We are generating a backtrace before the function has changed the frame pointer
            // 3. We are generating a backtrace after the function has called (and returned from)
            //      another function. This case is indistinguishable from 1 without human intervention
            let mut prev_fp;
            let start_frame = if current_frame.return_address != current_lr {
                prev_fp = std::ptr::null_mut();
                None
            } else {
                let entry = BacktraceEntry::new(std::ptr::NonNull::new(current_fp).unwrap());
                prev_fp = current_fp;
                current_fp = entry.frame.previous_frame;
                Some(entry)
            };

            // count the current entry as one of our max count
            limit -= 1;

            // create our backtrace vector
            let mut entries = [None; 33];

            let mut count = 0;
            while limit > 0 {
                // check if the frame pointer is null, if so we are done with the backtrace
                if current_fp.is_null() {
                    break;
                }
                
                // check if the previous frame pointer is equal to our current one
                // if so, we are going to be recursive so we might as well just end
                if prev_fp == current_fp {
                    entries[count] = Some(Err(BacktraceError::RecursiveFramePointer));
                    count += 1;
                    break;
                }

                let entry = BacktraceEntry::new(std::ptr::NonNull::new(current_fp).unwrap());

                // move forwards in the list
                prev_fp = current_fp;
                current_fp = entry.frame.previous_frame;

                // push current entry
                entries[count] = Some(Ok(entry));
                count += 1;

                limit -= 1;
            }

            // if we reached our limit then we should push an error to reflect that
            if limit == 0 {
                entries[count] = Some(Err(BacktraceError::BacktraceLimitReached));
            }

            Ok(Self {
                current_frame: start_frame,
                current_lr,
                backtrace: entries
            })
        }
    }

    /// Builds a new callstack backtrace based on the [`contexts::InlineCtx`]
    /// 
    /// # Arguments
    /// * `ctx` - The inline hook context
    /// * `limit` - The maximum number of stack frames to move back through
    /// 
    /// # Returns
    /// * `Ok(Backtrace)` - A successfully created backtrace
    /// * `Err(BacktraceError)` - A failed backtrace
    pub fn new_from_legacy_inline_ctx(ctx: &contexts::LegacyInlineCtx, limit: usize) -> Result<Self, BacktraceError> {
        Self::new(ctx.registers[29].x() as _, ctx.registers[30].x(), limit)
    }

    /// Builds a new callstack backtrace based on the [`contexts::ExInlineCtx`]
    /// 
    /// # Arguments
    /// * `ctx` - The extended inline hook context
    /// * `limit` - The maximum number of stack frames to move back through
    /// 
    /// # Returns
    /// * `Ok(Backtrace)` - A successfully created backtrace
    /// * `Err(BacktraceError)` - A failed backtrace
    pub fn new_from_inline_ctx(ctx: &contexts::InlineCtx, limit: usize) -> Result<Self, BacktraceError> {
        Self::new(ctx.registers[29].x() as _, ctx.registers[30].x(), limit)
    }
}

impl fmt::Display for Backtrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Current LR: {}", Self::get_formatted_addr_(self.current_lr, f.alternate()))?;
        let mut current = 0;
        if let Some(current_frame) = self.current_frame.as_ref() {
            writeln!(f, "      [{:02}]: {}", current, Self::get_formatted_addr_(current_frame.frame.return_address, f.alternate()))?;
            current += 1;
        }
        for entry in self.backtrace.iter() {
            match entry {
                Some(Ok(entry)) => writeln!(
                    f,
                    "      [{:02}]: {}",
                    current,
                    Self::get_formatted_addr_(entry.frame.return_address, f.alternate())
                )?,
                Some(Err(e)) => writeln!(
                    f,
                    "      [{:02}]: {}",
                    current,
                    e
                )?,
                None => break,
            }
            current += 1;
        }
        Ok(())
    }
}



#[cfg(feature = "static-module")]
impl Backtrace {
    pub fn write_formatted_addr<W: std::io::Write>(writer: &mut W, address: u64) -> std::io::Result<()> {
        if let Some(object) = crate::rtld::find_module_for_address(address) {
            let module_offset = address - object.module_base as u64;
            let name = object.get_module_name().unwrap_or("unknown");
            if let Some((sym_name, start)) = object.find_symbol_for_address(address) {
                let symbol_offset = address - start;
                write!(writer, "{:016x} ({} + {:#x}) ({} + {:#x})", address, name, module_offset, sym_name, symbol_offset)
            } else {
                write!(writer, "{:016x} ({} + {:#x})", address, name, module_offset)
            }
        } else {
            write!(writer, "{:016x}", address)
        }
    }

    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write!(writer, "Current LR: ")?;
        Self::write_formatted_addr(writer, self.current_lr)?;
        writeln!(writer)?;
        let mut current = 0;
        if let Some(current_frame) = self.current_frame.as_ref() {
            write!(writer, "      [{:02}]: ", current)?;
            Self::write_formatted_addr(writer, current_frame.frame.return_address)?;
            writeln!(writer)?;
            current += 1;
        }
        for entry in self.backtrace.iter() {
            match entry {
                Some(Ok(entry)) => {
                    write!(
                        writer,
                        "      [{:02}]: ",
                        current
                    )?;
                    Self::write_formatted_addr(writer, entry.frame.return_address)?;
                    writeln!(writer)?;
                },
                Some(Err(e)) => writeln!(
                    writer,
                    "      [{:02}]: {}",
                    current,
                    e
                )?,
                None => break,
            }
            current += 1;
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! get_backtrace {
    () => {
        get_backtrace!(32)
    };
    ($limit:expr) => {{
        let fp: *mut ::skyline::hooks::StackFrame;
        let lr: u64;

        std::arch::asm!(r#"
            mov {}, x29
            mov {}, x30
        "#, out(reg) fp, out(reg) lr);

        ::skyline::hooks::Backtrace::new(fp as _, lr, $limit)
    }}
}

pub use get_backtrace;