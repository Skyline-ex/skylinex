//! This module is for generating functions/hooks that are installed without JIT
//! 
//! These hooks have to be generated manually and they need their own enabling and trampoline
//! flags.
//! 
use proc_macro2::TokenStream;

use crate::attrs::HookAttributes;

use super::HookKind;

/// Assembly code to backup the CPU registers to a region on the stack which is reserved
/// to at least 0x100 in size
static CPU_REGISTER_BACKUP: &'static str = { r#"
    stp  x0,  x1, [sp, #0x00]
    stp  x2,  x3, [sp, #0x10]
    stp  x4,  x5, [sp, #0x20]
    stp  x6,  x7, [sp, #0x30]
    stp  x8,  x9, [sp, #0x40]
    stp x10, x11, [sp, #0x50]
    stp x12, x13, [sp, #0x60]
    stp x14, x15, [sp, #0x70]
    stp x16, x17, [sp, #0x80]
    stp x18, x19, [sp, #0x90]
    stp x20, x21, [sp, #0xA0]
    stp x22, x23, [sp, #0xB0]
    stp x24, x25, [sp, #0xC0]
    stp x26, x27, [sp, #0xD0]
    stp x28, x29, [sp, #0xE0]
    str x30, [sp, #0xF0]
"# };

/// Assembly code to backup the FPU registers to the same stack region as [`CPU_REGISTER_BACKUP`]
static FPU_REGISTER_BACKUP: &'static str = { r#"
    stp  q0,  q1, [sp, #0x100]
    stp  q2,  q3, [sp, #0x120]
    stp  q4,  q5, [sp, #0x140]
    stp  q6,  q7, [sp, #0x160]
    stp  q8,  q9, [sp, #0x180]
    stp q10, q11, [sp, #0x1A0]
    stp q12, q13, [sp, #0x1C0]
    stp q14, q15, [sp, #0x1E0]
    stp q16, q17, [sp, #0x200]
    stp q18, q19, [sp, #0x220]
    stp q20, q21, [sp, #0x240]
    stp q22, q23, [sp, #0x260]
    stp q24, q25, [sp, #0x280]
    stp q26, q27, [sp, #0x2A0]
    stp q28, q29, [sp, #0x2C0]
    stp q30, q31, [sp, #0x2E0]
"#};

/// Assembly code to restore the CPU registers from the stack
static CPU_REGISTER_RESTORE: &'static str = { r#"
    ldp  x0,  x1, [sp, #0x00]
    ldp  x2,  x3, [sp, #0x10]
    ldp  x4,  x5, [sp, #0x20]
    ldp  x6,  x7, [sp, #0x30]
    ldp  x8,  x9, [sp, #0x40]
    ldp x10, x11, [sp, #0x50]
    ldp x12, x13, [sp, #0x60]
    ldp x14, x15, [sp, #0x70]
    ldp x16, x17, [sp, #0x80]
    ldp x18, x19, [sp, #0x90]
    ldp x20, x21, [sp, #0xA0]
    ldp x22, x23, [sp, #0xB0]
    ldp x24, x25, [sp, #0xC0]
    ldp x26, x27, [sp, #0xD0]
    ldp x28, x29, [sp, #0xE0]
    ldr x30, [sp, #0xF0]
"# };

/// Assembly code to restore the FPU registers from the stack
static FPU_REGISTER_RESTORE: &'static str = { r#"
    ldp  q0,  q1, [sp, #0x100]
    ldp  q2,  q3, [sp, #0x120]
    ldp  q4,  q5, [sp, #0x140]
    ldp  q6,  q7, [sp, #0x160]
    ldp  q8,  q9, [sp, #0x180]
    ldp q10, q11, [sp, #0x1A0]
    ldp q12, q13, [sp, #0x1C0]
    ldp q14, q15, [sp, #0x1E0]
    ldp q16, q17, [sp, #0x200]
    ldp q18, q19, [sp, #0x220]
    ldp q20, q21, [sp, #0x240]
    ldp q22, q23, [sp, #0x260]
    ldp q24, q25, [sp, #0x280]
    ldp q26, q27, [sp, #0x2A0]
    ldp q28, q29, [sp, #0x2C0]
    ldp q30, q31, [sp, #0x2E0]
"# };

/// The context for generating the manual components of a symbol hook
struct ManualHookContext {
    /// The identifier of the user provided function
    base_ident:       syn::Ident,

    /// The identifier for the trampoline global
    trampoline_ident: syn::Ident,

    /// The identifier for the global flag for enabling/disabling the hook
    is_enabled_ident: syn::Ident,

    /// The name of the assembly function
    manual_ident:     syn::Ident,

    /// The name of the assembly jump-to-trampoline label
    trampoline_name:  String,
}

impl ManualHookContext {
    /// Constructs a new context from the given base identifier and the hook kind
    pub fn new(base: syn::Ident, kind: HookKind) -> Self {
        Self {
            trampoline_ident: quote::format_ident!("__skex_codegen_{}_{}_trampoline", base, kind.as_str()),
            is_enabled_ident: quote::format_ident!("__skex_codegen_{}_{}_is_enabled", base, kind.as_str()),
            manual_ident: quote::format_ident!("__skex_codegen_{}_manual_{}", base, kind.as_str()),
            trampoline_name: format!("__skex_codegen_{}_{}_jump_to_trampoline", base, kind.as_str()),
            base_ident: base
        }
    }
}

fn write_callback_assembly(ctx: &ManualHookContext) -> String {
    // {0}: The name of the user function provided during the callback
    // {1}: The name of our manual hook
    // {2}: The name of our "is enabled" global
    // {3}: The name of our trampoline label that we jump to
    // {4}: The CPU register backup code
    // {5}: The FPU register backup code
    // {6}: The CPU register restore code
    // {7}: The FPU register restore code
    // {8}: The name of our trampoline global
    format!(
    r#"
        .section .text.{0}, "ax", %progbits
        .global {1}
        .type {1}, %function
        .align 2
        .cfi_startproc
        {1}:
            // This is for PIC (Position Independent Code), since our 
            // globals are stored in the global offset table (got)
            adrp x16, :got:{2}
            ldr x16, [x16, :got_lo12:{2}]
            ldr w16, [x16]
            tbz w16, #0x0, {3}

            sub sp, sp, #0x300

            {4}

            add x0, sp, #0x300
            str x0, [sp, #0xF8]

            {5}

            ldr x0, [sp]

            bl {0}

            {6}

            {7}

            add sp, sp, #0x300
        {3}:
            // If our hook is not enabled, then don't even run the function and jump to the next one
            adrp x16, :got:{8}
            ldr x16, [x16, :got_lo12:{8}]
            ldr x16, [x16]
            br x16
        .cfi_endproc
    "#,
        ctx.base_ident,
        ctx.manual_ident,
        ctx.is_enabled_ident,
        ctx.trampoline_name,
        CPU_REGISTER_BACKUP,
        FPU_REGISTER_BACKUP,
        CPU_REGISTER_RESTORE,
        FPU_REGISTER_RESTORE,
        ctx.trampoline_ident,
    )
}

fn write_inline_assembly(ctx: &ManualHookContext) -> String {
    // {0}: The name of the user function provided during the callback
    // {1}: The name of our manual hook
    // {2}: The name of our "is enabled" global
    // {3}: The name of our trampoline label that we jump to
    // {4}: The CPU register backup code
    // {5}: The FPU register backup code
    // {6}: The CPU register restore code
    // {7}: The FPU register restore code
    // {8}: The name of our trampoline global
    format!(
    r#"
        .section .text.{0}, "ax", %progbits
        .global {1}
        .type {1}, %function
        .align 2
        .cfi_startproc
        {1}:
            // This is for PIC (Position Independent Code), since our 
            // globals are stored in the global offset table (got)
            adrp x16, :got:{2}
            ldr x16, [x16, :got_lo12:{2}]
            ldr w16, [x16]
            tbz w16, #0x0, {3}

            sub sp, sp, #0x300

            {4}

            add x0, sp, #0x300
            str x0, [sp, #0xF8]

            {5}

            mov x0, sp

            bl {0}

            {6}

            {7}

            add sp, sp, #0x300
        {3}:
            // If our hook is not enabled, then don't even run the function and jump to the next one
            adrp x16, :got:{8}
            ldr x16, [x16, :got_lo12:{8}]
            ldr x16, [x16]
            br x16
        .cfi_endproc
    "#,
        ctx.base_ident,
        ctx.manual_ident,
        ctx.is_enabled_ident,
        ctx.trampoline_name,
        CPU_REGISTER_BACKUP,
        FPU_REGISTER_BACKUP,
        CPU_REGISTER_RESTORE,
        FPU_REGISTER_RESTORE,
        ctx.trampoline_ident,
    )
}

fn write_legacy_inline_assembly(ctx: &ManualHookContext) -> String {
    // {0}: The name of the user function provided during the callback
    // {1}: The name of our manual hook
    // {2}: The name of our "is enabled" global
    // {3}: The name of our trampoline label that we jump to
    // {4}: The CPU register backup code
    // {5}: The CPU register restore code
    // {6}: The name of our trampoline global
    format!(
    r#"
        .section .text.{0}, "ax", %progbits
        .global {1}
        .type {1}, %function
        .align 2
        .cfi_startproc
        {1}:
            // This is for PIC (Position Independent Code), since our 
            // globals are stored in the global offset table (got)
            adrp x16, :got:{2}
            ldr x16, [x16, :got_lo12:{2}]
            ldr w16, [x16]
            tbz w16, #0x0, {3}

            sub sp, sp, #0x100

            {4}

            mov x0, sp

            bl {0}

            {5}

            add sp, sp, #0x100
        {3}:
            // If our hook is not enabled, then don't even run the function and jump to the next one
            adrp x16, :got:{6}
            ldr x16, [x16, :got_lo12:{6}]
            ldr x16, [x16]
            br x16
        .cfi_endproc
    "#,
        ctx.base_ident,
        ctx.manual_ident,
        ctx.is_enabled_ident,
        ctx.trampoline_name,
        CPU_REGISTER_BACKUP,
        CPU_REGISTER_RESTORE,
        ctx.trampoline_ident,
    )
}

fn write_hook_assembly(ctx: &ManualHookContext) -> String {
    // {0}: The name of the user function provided during the callback
    // {1}: The name of our manual hook
    // {2}: The name of our "is enabled" global
    // {3}: The name of our trampoline label that we jump to
    // {4}: The name of our trampoline global
    format!(
    r#"
        .section .text.{0}, "ax", %progbits
        .global {1}
        .type {1}, %function
        .align 2
        .cfi_startproc
        {1}:
            // This is for PIC (Position Independent Code), since our 
            // globals are stored in the global offset table (got)
            adrp x16, :got:{2}
            ldr x16, [x16, :got_lo12:{2}]
            ldr w16, [x16]
            tbz w16, #0x0, {3}
            b {0}

        {3}:
            // If our hook is not enabled, then don't even run the function and jump to the next one
            adrp x16, :got:{4}
            ldr x16, [x16, :got_lo12:{4}]
            ldr x16, [x16]
            br x16
        .cfi_endproc
    "#,
        ctx.base_ident,
        ctx.manual_ident,
        ctx.is_enabled_ident,
        ctx.trampoline_name,
        ctx.trampoline_ident,
    )
}

fn generate_install_fn(ctx: &ManualHookContext, args: HookAttributes, kind: HookKind) -> syn::Result<TokenStream> {
    // Attempt to get the name of the skyline crate as imported by the user
    let skyline = crate::get_skyline_crate_name()?;
    
    // Extract our needed elements from the context
    let ManualHookContext {
        manual_ident,
        trampoline_ident,
        ..
    } = ctx;

    // Get the hook kind as a path
    let kind = kind.to_path(&skyline);

    // Here, we are performing the check to see if the provided expression is a string
    // If it is, then we are to assume that we are installing this on a symbol which is not yet resolved.
    let function_expr = &args.style.value;

    let future_symbol = match function_expr {
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Str(str) => Some(str),
            _ => None
        },
        _ => None
    };

    // Our call to the FFI export is different depending on whether or not we are using
    // future symbol
    let ffi_function_call = if let Some(future_symbol) = future_symbol {
        // If we are using a future symbol, we are going to very slightly modify it
        // so that it is null-terminated, since this is C FFI
        let future_symbol = syn::LitStr::new(
            &format!("{}\0", future_symbol.value()),
            future_symbol.span()
        );

        // Generate the future symbol hook call
        quote::quote! {
            #skyline::hooks::ffi::skex_hooks_install_on_symbol_future(
                self_object,
                #future_symbol.as_ptr(),
                #manual_ident as *const (),
                &mut #trampoline_ident as *mut u64 as *mut *mut (),
                #kind
            );
        }
    } else {
        // We don't have to do anything special, just use the user expression
        //
        // Note that since we are *not* a JIT hook, we don't have to attempt to evaluate an offset from a module or anything
        // and it is up to the user to know that if they are providing non-function under `replace` then they need to pass `force_jit`
        quote::quote! {
            #skyline::hooks::ffi::skex_hooks_install_on_symbol(
                self_object,
                (#function_expr) as *const (),
                #manual_ident as *const (),
                &mut #trampoline_ident as *mut u64 as *mut *mut (),
                #kind
            );
        }
    };

    // We have to extern "C" the manual ident since it is declared in assembly
    Ok(quote::quote! {
        pub fn install() {
            extern "C" {
                fn #manual_ident();
            }

            unsafe {
                let self_object = #skyline::rtld::get_module_for_self()
                        .expect("Should be able to find the ModuleObject for ourself");
                let self_object = self_object as *const #skyline::rtld::ModuleObject as *mut #skyline::rtld::ModuleObject;
                #ffi_function_call
            }
        }
    })
}

fn generate_uninstall_fn(ctx: &ManualHookContext) -> syn::Result<TokenStream> {
    let skyline = crate::get_skyline_crate_name()?;
    let manual_ident = &ctx.manual_ident;

    // Very simple uninstall function, just to wrap up the FFI call
    Ok(quote::quote! {
        pub fn uninstall() {
            extern "C" {
                fn #manual_ident();
            }

            unsafe {
                #skyline::hooks::ffi::skex_hooks_uninstall_from_symbol(#manual_ident as *const ());
            }
        }
    })
}

fn generate_enable_fn(ctx: &ManualHookContext) -> TokenStream {
    // This one doesn't return a result since there is no FFI here
    // since we are the ones in control over the is enabled global
    let is_enabled_ident = &ctx.is_enabled_ident;

    quote::quote! {
        pub fn enable() {
            unsafe {
                #is_enabled_ident = true;
            }
        }
    }
}

fn generate_disable_fn(ctx: &ManualHookContext) -> TokenStream {
    // This one doesn't return a result since there is no FFI here
    // since we are the ones in control over the is enabled global
    let is_enabled_ident = &ctx.is_enabled_ident;

    quote::quote! {
        pub fn disable() {
            unsafe {
                #is_enabled_ident = false;
            }
        }
    }
}

pub fn make_symbol_hook(
    mut user_function: syn::ItemFn,
    args: HookAttributes,
    kind: HookKind
) -> syn::Result<TokenStream> {
    // Construct a new hook context
    let ctx = ManualHookContext::new(user_function.sig.ident.clone(), kind);

    // Generate the assembly string to use for the global asm
    let asm_string = match kind {
        HookKind::Callback => write_callback_assembly(&ctx),
        HookKind::Inline => write_inline_assembly(&ctx),
        HookKind::LegacyInline => write_legacy_inline_assembly(&ctx),
        HookKind::Hook => {
            // If we are a hook, we are also inserting the original macros/function
            super::push_original_utils(&mut user_function, &ctx.base_ident, &ctx.trampoline_ident)?;
            write_hook_assembly(&ctx)
        }
    };

    // Convert the string into a string literal for tokenization
    let manual_asm = syn::LitStr::new(&asm_string, user_function.sig.ident.span());

    // Get all of the module functions
    let install_fn = generate_install_fn(&ctx, args, kind)?;
    let uninstall_fn = generate_uninstall_fn(&ctx)?;
    let enable_fn = generate_enable_fn(&ctx);
    let disable_fn = generate_disable_fn(&ctx);

    // Extract the required context elements to make the module
    let ManualHookContext {
        base_ident,
        trampoline_ident,
        is_enabled_ident,
        ..
    } = &ctx;

    // Use the user provided visibility on the hook
    let vis = &user_function.vis;

    // Create the module
    Ok(quote::quote! {
        #vis mod #base_ident {
            use super::*;

            #[no_mangle]
            #[allow(non_upper_case_globals)]
            #[allow(non_snake_case)]
            pub(super) static mut #trampoline_ident: u64 = 0;

            #[no_mangle]
            #[allow(non_upper_case_globals)]
            #[allow(non_snake_case)]
            static mut #is_enabled_ident: bool = true;

            #install_fn

            #uninstall_fn

            #enable_fn

            #disable_fn
        }

        std::arch::global_asm!(#manual_asm);

        #user_function
    })
}