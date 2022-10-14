use proc_macro2::{TokenStream, Span};

use crate::attrs::{ModuleArg, HookAttributes, HookStyle, KnownModule, kw};

use super::HookKind;

struct HookContext {
    base_ident: syn::Ident,
    trampoline_ident: syn::Ident,
}

impl HookContext {
    pub fn new(base_ident: syn::Ident, kind: HookKind) -> Self {
        Self {
            trampoline_ident: quote::format_ident!("__skex_codegen_{}_{}_trampoline", base_ident, kind.as_str()),
            base_ident
        }
    }
}

impl KnownModule {
    fn to_path(&self, skyline: &syn::Ident) -> syn::Path {
        match self {
            Self::Rtld(_) => syn::parse_quote!(#skyline::memory::StaticModule::Rtld),
            Self::Main(_) => syn::parse_quote!(#skyline::memory::StaticModule::Main),
            Self::Skyline(_) => syn::parse_quote!(#skyline::memory::StaticModule::SkylineEx),
            Self::Sdk(_) => syn::parse_quote!(#skyline::memory::StaticModule::Sdk),
        }
    }
}

fn evaluate_hooking_expression(attrs: &HookAttributes, ctx: &HookContext, kind: HookKind) -> syn::Result<TokenStream> {
    // We are evaluating the expression, regardless of whether or not it is an absolute
    // or a relative expression, so get it first.
    let offset_expr = &attrs.style.value;

    // If it is an absolute expression just put it down and leave
    if matches!(&attrs.style.key, HookStyle::Symbol) {
        return Ok(quote::quote!(#offset_expr));
    }

    // Extract the module argument from the attributes, and if it does not exist
    // then we should use the main module as the default
    let module = if let Some(module) = &attrs.module {
        module.value.clone()
    } else {
        ModuleArg::ByKnown(KnownModule::Main(kw::main(Span::call_site())))
    };

    // Get the skyline crate ahead of time
    let skyline = crate::get_skyline_crate_name()?;

    match module {
        // If it is a known module, then we can simply get absolute address by using the known module FFI
        ModuleArg::ByKnown(known) => {
            let path = known.to_path(&skyline);
            Ok(quote::quote! {
                &#skyline::memory::ffi::skex_memory_get_known_static_module(#path).text()[(#offset_expr) as usize] as *const u8 as *const ()
            })
        },

        // Otherwise, it can either be a dynamic module or a static module. If it is a static module, then we can let
        // the remainder of our hook generation install it, but if it is dynamic we need to take a different
        // installation path.
        ModuleArg::ByName(name) => {
            // Make sure the name is null terminated
            let name = syn::LitStr::new(
                &format!("{}\0", name.value()),
                name.span()
            );

            // Get all of the idents and paths we will need to generate our code
            let base_ident = &ctx.base_ident;
            let trampoline_ident = &ctx.trampoline_ident;
            let kind = kind.to_path(&skyline);

            // First we try using the static module by name, and then if that doesn't work
            // we will fallback on the dynamic module hooking.
            Ok(quote::quote! {
                if let Some(module) = &#skyline::memory::ffi::skex_memory_get_static_module_by_name(#name.as_ptr()) {
                    &module.text()[(#offset_expr) as usize] as *const u8
                } else {
                    #skyline::hooks::ffi::skex_hooks_install_on_dynamic_load(
                        (#offset_expr) as usize,
                        #base_ident as *const (),
                        &mut #trampoline_ident as *mut u64 as *mut *mut (),
                        #name.as_ptr(),
                        #kind
                    );
                    return;
                }
            })
        }
    }
}

fn evaluate_hooking_expression_for_set_enable(attrs: &HookAttributes) -> syn::Result<TokenStream> {
    // We are evaluating the expression, regardless of whether or not it is an absolute
    // or a relative expression, so get it first.
    let offset_expr = &attrs.style.value;

    // If it is an absolute expression just put it down and leave
    if matches!(&attrs.style.key, HookStyle::Symbol) {
        return Ok(quote::quote!(#offset_expr));
    }

    // Extract the module argument from the attributes, and if it does not exist
    // then we should use the main module as the default
    let module = if let Some(module) = &attrs.module {
        module.value.clone()
    } else {
        ModuleArg::ByKnown(KnownModule::Main(kw::main(Span::call_site())))
    };

    // Get the skyline crate ahead of time
    let skyline = crate::get_skyline_crate_name()?;

    match module {
        // If it is a known module, then we can simply get absolute address by using the known module FFI
        ModuleArg::ByKnown(known) => {
            let path = known.to_path(&skyline);
            Ok(quote::quote! {
                &#skyline::memory::ffi::skex_memory_get_known_static_module(#path).text()[(#offset_expr) as usize] as *const u8 as *const ()
            })
        },

        // Otherwise, it can either be a dynamic module or a static module. If it is a static module, then we can let
        // the remainder of our hook generation install it, but if it is dynamic we need to take a different
        // installation path.
        ModuleArg::ByName(name) => {
            // Make sure the name is null terminated
            let name = syn::LitStr::new(
                &format!("{}\0", name.value()),
                name.span()
            );

            // First we try using the static module by name, and then if that doesn't work
            // we will fallback on the dynamic module hooking.
            Ok(quote::quote! {
                if let Some(module) = &#skyline::memory::ffi::skex_memory_get_static_module_by_name(#name.as_ptr()) {
                    &module.text()[(#offset_expr) as usize] as *const u8
                } else {
                    let __non_null_name = #name.split_at(#name.len() - 1).0;
                    if let Some(module) = #skyline::rtld::find_module_by_name(#name.split_at(#name.len() - 1).0) {
                        module.module_base.add((#offset_expr) as usize) as *const u8
                    } else {
                        panic!("Dynamic module \"{}\" is not currently loaded, the hook state cannot be changed!", __non_null_name);
                    }
                }
            })
        }
    }
}

fn generate_install_fn(attrs: &HookAttributes, ctx: &HookContext, kind: HookKind) -> syn::Result<TokenStream> {
    let evaluation = evaluate_hooking_expression(attrs, ctx, kind)?;
    let skyline = crate::get_skyline_crate_name()?;

    let trampoline_ident = &ctx.trampoline_ident;
    let base_ident = &ctx.base_ident;
    let kind = kind.to_path(&skyline);

    Ok(quote::quote! {
        pub fn install() {
            unsafe {
                let __location = #evaluation;
                *(&mut #trampoline_ident as *mut u64 as *mut *const ()) = #skyline::hooks::ffi::skex_hooks_install(
                    __location as *const (),
                    #base_ident as *const (),
                    #kind
                );
            }
        }
    })
}

fn generate_uninstall_fn(ctx: &HookContext) -> syn::Result<TokenStream> {
    let skyline = crate::get_skyline_crate_name()?;
    let base_ident = &ctx.base_ident;

    Ok(quote::quote! {
        pub fn uninstall() {
            unsafe {
                #skyline::hooks::ffi::skex_hooks_uninstall(#base_ident as *const ());
            }
        }
    })
}

fn generate_enable_fn(ctx: &HookContext, args: &HookAttributes) -> syn::Result<TokenStream> {
    let skyline = crate::get_skyline_crate_name()?;
    let base_ident = &ctx.base_ident;

    let expr = evaluate_hooking_expression_for_set_enable(args)?;

    Ok(quote::quote! {
        pub fn enable() {
            unsafe {
                let __expr = #expr;
                #skyline::hooks::ffi::skex_hooks_set_enable(#base_ident as *const (), __expr as *const (), true);
            }
        }
    })
}

fn generate_disable_fn(ctx: &HookContext, args: &HookAttributes) -> syn::Result<TokenStream> {
    let skyline = crate::get_skyline_crate_name()?;
    let base_ident = &ctx.base_ident;

    let expr = evaluate_hooking_expression_for_set_enable(args)?;

    Ok(quote::quote! {
        pub fn disable() {
            unsafe {
                let __expr = #expr;
                #skyline::hooks::ffi::skex_hooks_set_enable(#base_ident as *const (), __expr as *const (), false);
            }
        }
    })
}

pub fn make_jit_hook(mut user_function: syn::ItemFn, args: HookAttributes, kind: HookKind) -> syn::Result<TokenStream> {
    let ctx = HookContext::new(user_function.sig.ident.clone(), kind);

    if matches!(kind, HookKind::Hook) {
        super::push_original_utils(&mut user_function, &ctx.base_ident, &ctx.trampoline_ident)?;
    }

    let install_fn = generate_install_fn(&args, &ctx, kind)?;
    let uninstall_fn = generate_uninstall_fn(&ctx)?;
    let enable_fn = generate_enable_fn(&ctx, &args)?;
    let disable_fn = generate_disable_fn(&ctx, &args)?;

    let base_ident = &ctx.base_ident;
    let trampoline_ident = &ctx.trampoline_ident;

    let vis = &user_function.vis;

    Ok(quote::quote! {
        #vis mod #base_ident {
            use super::*;

            #[allow(non_upper_case_globals)]
            #[allow(non_snake_case)]
            pub(super) static mut #trampoline_ident: u64 = 0;

            #install_fn

            #uninstall_fn

            #enable_fn

            #disable_fn
        }

        #user_function
    })
}