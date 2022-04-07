use super::contexts;

use thiserror::Error;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct StackFrame {
    pub previous_frame: *mut StackFrame,
    pub return_address: u64,
}

#[derive(Error, Debug)]
pub enum BacktraceError {
    #[error("The initial frame pointer is null")]
    InitialFPNull,

    #[error("The backtrace is recursive and the frame pointer points to itself")]
    RecursiveFramePointer,

    #[error("The backtrace is longer than the provided limit")]
    BacktraceLimitReached
}

#[derive(Debug)]
pub struct BacktraceEntry {
    ptr: *mut StackFrame,
    frame: StackFrame,
}

impl BacktraceEntry {
    /// Creates a backtrace entry from a given stack frame pointer
    pub fn new(ptr: *mut StackFrame) -> Self {
        Self {
            ptr,
            frame: unsafe { *ptr }
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
    backtrace: Vec<Result<BacktraceEntry, BacktraceError>>,
}

impl Backtrace {

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
    pub fn new(mut current_fp: *mut StackFrame, current_lr: u64, mut limit: usize) -> Result<Self, BacktraceError> {
        // if the frame pointer is null then we can't really generate a stack trace any more meaningful
        // than the provided lr, which the caller should already have
        if current_fp.is_null() {
            return Err(BacktraceError::InitialFPNull);
        }

        unsafe {
            let current_frame = *current_fp;
            // If the current stack frame's LR is not the same as what
            // was provided, we can assume that the backtrace is being generated in 
            // one of two contexts:
            // 1. The surrounding function does not make use of the frame pointer and does not
            //      push it, which usually means that they aren't calling any other functions
            //      You can see an example of this here: https://godbolt.org/z/Weza98z3q
            //      Here, `main` pushes the frame pointer, calls `something` which pushes the frame pointer
            //      which then calls `something2`, which uses the stack but doesn't push the frame pointer
            //      since it doesn't need to worry about any internal function calls messing up
            //      the x30 register (which is used as the return address)
            // 2. We are generating a backtrace before the function has changed the frame pointer
            let mut prev_fp;
            let start_frame = if current_frame.return_address != current_lr {
                prev_fp = std::ptr::null_mut();
                None
            } else {
                let entry = BacktraceEntry::new(current_fp);
                prev_fp = current_fp;
                current_fp = entry.frame.previous_frame;
                Some(BacktraceEntry::new(current_fp))
            };

            // count the current entry as one of our max count
            limit -= 1;

            // create our backtrace vector
            let mut entries = Vec::with_capacity(limit);

            while limit > 0 {
                // check if the frame pointer is null, if so we are done with the backtrace
                if current_fp.is_null() {
                    break;
                }
                
                // check if the previous frame pointer is equal to our current one
                // if so, we are going to be recursive so we might as well just end
                if prev_fp == current_fp {
                    entries.push(Err(BacktraceError::RecursiveFramePointer));
                    break;
                }

                let entry = BacktraceEntry::new(current_fp);

                // move forwards in the list
                prev_fp = current_fp;
                current_fp = entry.frame.previous_frame;

                // push current entry
                entries.push(Ok(entry));

                limit -= 1;
            }

            // if we reached our limit then we should push an error to reflect that
            if limit == 0 {
                entries.push(Err(BacktraceError::BacktraceLimitReached));
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
    pub fn new_from_inline_ctx(ctx: &contexts::InlineCtx, limit: usize) -> Result<Self, BacktraceError> {
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
    pub fn new_from_ex_inline_ctx(ctx: &contexts::ExInlineCtx, limit: usize) -> Result<Self, BacktraceError> {
        Self::new(ctx.registers[29].x() as _, ctx.registers[30].x(), limit)
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

        asm!(r#"
            mov {}, x29
            mov {}, x30
        "#, out(reg) fp, out(reg) lr);

        ::skyline::hooks::Backtrace::new(fp as _, lr, $limit)
    }}
}