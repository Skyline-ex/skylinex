mod backtrace;
mod contexts;
mod registers;

pub use backtrace::*;
pub use contexts::*;
pub use registers::*;

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum HookType {
    Callback,
    Inline,
    LegacyInline,
    Hook,
}

#[doc(hidden)]
pub mod ffi {
    extern "C" {
        pub fn skex_hooks_install_on_symbol(
            host_object: *mut crate::rtld::ModuleObject,
            function: *const (),
            replace: *const (),
            out_trampoline: *mut *mut (),
            hook_ty: super::HookType
        );

        pub fn skex_hooks_install_on_symbol_future(
            host_object: *mut crate::rtld::ModuleObject,
            name: *const u8,
            replace: *const (),
            out_trampoline: *mut *mut (),
            hook_ty: super::HookType
        );

        pub fn skex_hooks_install(
            symbol: *const (),
            replace: *const (), 
            hook_ty: super::HookType
        ) -> *const ();

        pub fn skex_hooks_install_on_dynamic_load(
            symbol_offset: usize,
            replace: *const (),
            out_trampoline: *mut *mut (),
            name: *const u8,
            hook_ty: super::HookType
        );

        pub fn skex_hooks_set_enable(user: *const (), symbol: *const (), enable: bool);

        pub fn skex_hooks_uninstall(user: *const ());
        
        pub fn skex_hooks_uninstall_from_symbol(user: *const ());
    }
}