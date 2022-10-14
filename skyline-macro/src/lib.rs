#![feature(proc_macro_diagnostic)]
#![feature(let_else)]
use proc_macro2::Span;
use proc_macro_crate::FoundCrate;
use proc_macro::TokenStream;

mod attrs;
mod hooks;

fn get_skyline_crate_name() -> syn::Result<syn::Ident> {
    match proc_macro_crate::crate_name("skyline") {
        Ok(FoundCrate::Itself) => Ok(syn::Ident::new("crate", Span::call_site())),
        Ok(FoundCrate::Name(named)) => Ok(syn::Ident::new(named.as_str(), Span::call_site())),
        Err(e) => Err(syn::Error::new(Span::call_site(), e)),
    }
}

#[proc_macro_attribute]
pub fn hook(attr: TokenStream, item: TokenStream) -> TokenStream {
    hooks::make_hook(attr, item, hooks::HookKind::Hook)
}

#[proc_macro_attribute]
pub fn inline_hook(attr: TokenStream, item: TokenStream) -> TokenStream {
    hooks::make_hook(attr, item, hooks::HookKind::Inline)
}

#[proc_macro_attribute]
pub fn legacy_inline_hook(attr: TokenStream, item: TokenStream) -> TokenStream {
    hooks::make_hook(attr, item, hooks::HookKind::LegacyInline)
}

#[proc_macro_attribute]
pub fn callback(attr: TokenStream, item: TokenStream) -> TokenStream {
    hooks::make_hook(attr, item, hooks::HookKind::Callback)
}

#[proc_macro_attribute]
pub fn shim(attr: TokenStream, item: TokenStream) -> TokenStream {
    hooks::make_shim(attr, item)
}

#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr: attrs::MainAttrs = syn::parse_macro_input!(attr);
    let mut item = syn::parse_macro_input!(item as syn::ItemFn);

    item.attrs.push(syn::parse_quote!(#[no_mangle]));
    item.sig.abi = Some(syn::parse_quote!(extern "C"));

    let name = attr.value.value();

    let asm_str = format!(r#"
    .section .nro_header
    .global __nro_header_start
    .global __module_start
    __module_start:
    .word 0
    .word _mod_header
    .word 0
    .word 0
    
    .section .rodata.module_name
        .word 0
        .word {}
        .ascii "{}"
    .section .rodata.mod0
    .global _mod_header
    _mod_header:
        .ascii "MOD0"
        .word __dynamic_start - _mod_header
        .word __bss_start - _mod_header
        .word __bss_end - _mod_header
        .word __eh_frame_hdr_start - _mod_header
        .word __eh_frame_hdr_end - _mod_header
        .word __nx_module_runtime - _mod_header // runtime-generated module object offset
    .global IS_NRO
    IS_NRO:
        .word 1
    
    .section .bss.module_runtime
    __nx_module_runtime:
    .space 0xD0
    "#,
    name.len(),
    name
    );

    quote::quote! {
        std::arch::global_asm!(#asm_str);
        #item
    }.into()
}