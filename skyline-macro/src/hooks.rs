use proc_macro2::{Span, TokenStream};
use syn::spanned::Spanned;

use super::attrs::*;

/// The kind of hook to install
#[derive(Copy, Clone)]
pub enum HookKind {
    Callback,
    Inline,
    LegacyInline,
    Hook
}

impl HookKind {
    /// Generates a path to be used during codegen for the hook kind
    pub fn to_path(&self, skyline: &syn::Ident) -> syn::Path {
        match self {
            Self::Callback => syn::parse_quote!(#skyline::hooks::HookType::Callback),
            Self::Inline => syn::parse_quote!(#skyline::hooks::HookType::Inline),
            Self::LegacyInline => syn::parse_quote!(#skyline::hooks::HookType::LegacyInline),
            Self::Hook => syn::parse_quote!(#skyline::hooks::HookType::Hook)
        }
    }

    /// Gets the hook kind as a string for ident creation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Callback => "callback",
            Self::Inline => "inline",
            Self::LegacyInline => "legacy_inline",
            Self::Hook => "hook"
        }
    }
}

/// Extracts the identifier from the provided argument
fn arg_to_name(input: &syn::FnArg) -> syn::Result<syn::Ident> {
    match input {
        // If the argument is a receiver, just return an ident for `this`
        syn::FnArg::Receiver(receiver) => {
            Ok(syn::Ident::new("this", receiver.span()))
        },
        // If it is a typed argument, make sure that the ident pattern is *not* a wild match,
        // if it is we can't really use it
        // TODO: Map `_` -> some new ident
        syn::FnArg::Typed(syn::PatType { pat, .. }) => match &**pat {
            syn::Pat::Ident(syn::PatIdent { ident, .. }) => Ok(ident.clone()),
            _ => Err(syn::Error::new(pat.span(), "invalid argument pattern"))
        }
    }
}

/// Converts the provided argument into one that can be used without warnings during codegen, i.e. removing `mut` and converting `self` -> `this`
fn convert_arg(input: &syn::FnArg) -> syn::Result<syn::FnArg> {
    match input {
        // If the argument is a receiver we need to go through the steps of converting it to a different
        // ident.
        // These can't be used for hooks but for `from_offset` calls it would be useful to have
        syn::FnArg::Receiver(receiver) => {
            let attrs = receiver.attrs.clone();
            let ident = syn::PatIdent {
                attrs: vec![],
                by_ref: None,
                mutability: None,
                ident: syn::Ident::new("this", receiver.span()),
                subpat: None
            };

            let mut path = syn::Path {
                leading_colon: None,
                segments: syn::punctuated::Punctuated::new()
            };

            path.segments.push(syn::PathSegment {
                ident: syn::Ident::new("Self", receiver.span()),
                arguments: syn::PathArguments::None
            });

            let ty = syn::Type::Path(syn::TypePath {
                qself: None,
                path
            });


            let ty = if let Some((and, lifetime)) = receiver.reference.as_ref() {
                syn::Type::Reference(syn::TypeReference {
                    and_token: and.clone(),
                    lifetime: lifetime.clone(),
                    mutability: receiver.mutability.clone(),
                    elem: Box::new(ty)
                })
            } else {
                ty
            };

            Ok(syn::FnArg::Typed(syn::PatType {
                attrs: attrs,
                pat: Box::new(syn::Pat::Ident(ident)),
                colon_token: syn::token::Colon(receiver.span()),
                ty: Box::new(ty)
            }))
        },
        syn::FnArg::Typed(ty) => match &*ty.pat {
            syn::Pat::Ident(_) => {
                let mut ty = ty.clone();
                match &mut *ty.pat {
                    syn::Pat::Ident(ident) => ident.mutability = None,
                    _ => unreachable!()
                }
                Ok(syn::FnArg::Typed(ty))
            },
            _ => Err(syn::Error::new(input.span(), "invalid argument specified"))
        }
    }
}

/// Checks if the hook should use the JIT hooking table
fn should_be_jit_hook(attrs: &HookAttributes) -> bool {
    // Helper function for unnecessary argument warning so that we don't duplicate code
    let try_emit_warning = || {
        if let Some(force_jit) = &attrs.force_jit {
            force_jit
                .span()
                .unwrap()
                .warning("unnecessary argument of 'force_jit' on hook which must utilize JIT anyways")
                .emit();
        }
    };

    // We need to use JIT if we are not using the `replace` keyword in the macro arguments,
    // so that is our first check
    if !matches!(&attrs.style.key, &HookStyle::Symbol) { 
        try_emit_warning();
        return true;
    }

    // Then we need to check on the expression. If it is a string, it means that it *has* to be a symbol hook.
    // Otherwise, if it is *not* a path then we should emit a diagnostic warning if they want to force JIT
    // since it already has to be JIT. We can't really do anything about a path that isn't a function
    // because we don't have access to type information in the proc macro.
    match &attrs.style.value {
        // First check if it is a literal
        syn::Expr::Lit(lit) => match &lit.lit {
            // If it is a string literal then we **have** to use a symbol hook
            // This kinda sucks because we can't use a global variable that is a string since
            // we don't have type info, but I'm sure the user would understand
            syn::Lit::Str(_) => {
                // Check if the force_jit flag is argument is provided, and if it is
                // emit a compiler *error* since it's an invalid argument here
                if let Some(force_jit) = &attrs.force_jit {
                    force_jit
                        .span()
                        .unwrap()
                        .error("invalid argument of 'force_jit' on hook that must be installed on a symbol")
                        .emit();
                }
                false
            },
            // Any other literal we should treat as needing JIT
            _ => {
                try_emit_warning();
                true
            }
        },
        // A path is the only one where we need to be able to force JIT
        syn::Expr::Path(_) => attrs.force_jit.is_some(),

        // Otherwise we know we are going to use JIT
        _ => {
            try_emit_warning();
            true
        }
    }
}

/// Emits a compiler error if user has provided a `replace` hooking style (which is an absolute expression)
/// with a `module` argument
fn error_module_on_replace(attrs: &HookAttributes) {
    // First ensure that the hooking style is a symbol/keyword of replace
    if !matches!(&attrs.style.key, HookStyle::Symbol) { return; }

    // Get the module argument, if it doesn't exist then there is no warning to emit
    let Some(module) = &attrs.module else {
        return;
    };

    let span = module.key.span();
    let span = span.join(module.equals.span()).unwrap();
    let span = span.join(module.value.span()).unwrap();

    span
        .unwrap()
        .error("invalid `module` argument on hook which uses absolute address -- did you mean `offset` instead of `replace`?")
        .emit();
}

fn push_original_utils(user_fn: &mut syn::ItemFn, base_ident: &syn::Ident, trampoline_ident: &syn::Ident) -> syn::Result<()> {
    // For compatibility reasons, we are going to provide both the `original!()` and `call_original!(...)`
    // macros, as well as a new function just called `original` which will serve the purposes of both

    // The arguments with the optional `mut` specifier removed on the ident
    let args = user_fn.sig.inputs
        .iter()
        .map(convert_arg)
        .collect::<syn::Result<Vec<_>>>()?;

    // The return value of the function
    let outputs = &user_fn.sig.output;

    let args_ = args.iter();

    // Insert the original! macro
    user_fn.block.stmts.insert(0, syn::parse_quote! {
        macro_rules! original {
            () => {
                if true {
                    #[allow(unused_unsafe)]
                    unsafe {
                        if #base_ident::#trampoline_ident == 0 {
                            panic!("Error calling function hook {}, original function not set.", stringify!(#base_ident));
                        } else {
                            std::mem::transmute::<_, extern "C" fn(#(#args_),*) #outputs>(#base_ident::#trampoline_ident as *const ())
                        }
                    }
                } else {
                    unreachable!()
                }
            }
        }
    });

    // Insert the call_original! macro
    user_fn.block.stmts.insert(1, syn::parse_quote! {
        macro_rules! call_original {
            ($($args:expr),* $(,)?) => {
                original!()($($args),*)
            }
        }
    });

    // Strip all of the names from their function arguments
    let names = args
        .iter()
        .map(arg_to_name)
        .collect::<syn::Result<Vec<_>>>()?
        .into_iter();
    
    let args = args.iter();

    // Insert the original function
    user_fn.block.stmts.insert(2, syn::parse_quote! {
        extern "C" fn original(#(#args),*) #outputs {
            call_original!(#(#names),*)
        }
    });

    Ok(())
}

mod jit_hooks;
mod symbol_hooks;

fn make_hook_internal(attrs: HookAttributes, mut user_fn: syn::ItemFn, kind: HookKind) -> proc_macro::TokenStream {
    let is_symbol_hook = !should_be_jit_hook(&attrs);

    error_module_on_replace(&attrs);

    // Change our signature ABI to be extern "C", so that we guarantee to be using the proper register layout
    // when getting called from C
    user_fn.sig.abi = Some(syn::Abi {
        extern_token: syn::token::Extern(Span::call_site()),
        name: Some(syn::LitStr::new("C", Span::call_site()))
    });

    // Push #[deny(improper_ctypes_definitions)] to the function so that
    // if the user provided a struct which is not C FFI safe then it becomes a compiler error,
    // since we just changed the ABI
    user_fn.attrs.push(syn::Attribute {
        pound_token: syn::token::Pound(Span::call_site()),
        style: syn::AttrStyle::Outer,
        bracket_token: syn::token::Bracket(Span::call_site()),
        path: syn::parse_quote!(deny),
        tokens: quote::quote!((improper_ctypes_definitions)),
    });

    // Push #[no_mangle] to the function so that we can locate the code in a decompiler easier for debugging,
    // and also for symbol hooks so the inline ASM can properly jump to it
    user_fn.attrs.push(syn::Attribute {
        pound_token: syn::token::Pound(Span::call_site()),
        style: syn::AttrStyle::Outer,
        bracket_token: syn::token::Bracket(Span::call_site()),
        path: syn::parse_quote!(no_mangle),
        tokens: TokenStream::new(),
    });

    let result = if is_symbol_hook {
        symbol_hooks::make_symbol_hook(user_fn, attrs, kind)
    } else {
        jit_hooks::make_jit_hook(user_fn, attrs, kind)
    };

    match result {
        Ok(stream) => stream.into(),
        Err(e) => e.into_compile_error().into()
    }

}

pub fn make_hook(attr: proc_macro::TokenStream, item: proc_macro::TokenStream, kind: HookKind) -> proc_macro::TokenStream {
    let attrs = syn::parse_macro_input!(attr as HookAttributes);
    let user_fn = syn::parse_macro_input!(item as syn::ItemFn);

    make_hook_internal(attrs, user_fn, kind)
}

pub fn make_shim(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let attrs = syn::parse_macro_input!(attr as HookAttributes);
    let mut user_fn = syn::parse_macro_input!(item as syn::ItemFn);

    let skyline = match crate::get_skyline_crate_name() {
        Ok(skyline) => skyline,
        Err(e) => return e.into_compile_error().into()
    };

    let mut stmts = vec![];
    std::mem::swap(&mut user_fn.block.stmts, &mut stmts);
    let stmts = stmts.into_iter();

    match &user_fn.sig.output {
        syn::ReturnType::Default => {
            user_fn.block.stmts.push(syn::parse_quote! {
                static ONCE: std::sync::Once = std::sync::Once::new();
            });
            user_fn.block.stmts.push(syn::parse_quote! {
                ONCE.call_once(|| { #(#stmts)* });
            });
        },
        syn::ReturnType::Type(_, ty) => {
            user_fn.block.stmts.push(syn::parse_quote! {
                static ONCE: #skyline::once_cell::sync::OnceCell<#ty> = #skyline::once_cell::sync::OnceCell::new();
            });
            user_fn.block.stmts.push(syn::parse_quote! {
                return *ONCE.get_or_init(|| { #(#stmts)* });
            });
        }
    }

    make_hook_internal(attrs, user_fn, HookKind::Hook)
}