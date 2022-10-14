use syn::{parse::Parse, spanned::Spanned};

pub mod kw {
    syn::custom_keyword!(module);
    syn::custom_keyword!(replace);
    syn::custom_keyword!(offset);
    syn::custom_keyword!(force_jit);
    syn::custom_keyword!(main);
    syn::custom_keyword!(nnSdk);
    syn::custom_keyword!(skyline);
    syn::custom_keyword!(nnrtld);
    syn::custom_keyword!(name);
}

pub struct KeyValue<Key: Parse, Value: Parse> {
    pub key: Key,
    pub equals: syn::Token![=],
    pub value: Value
}

impl<Key: Parse, Value: Parse> Parse for KeyValue<Key, Value> {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let key = input.parse()?;
        let equals = input.parse()?;
        let value = input.parse()?;
        Ok(Self { key, equals, value })
    }
}

#[derive(Clone)]
pub enum KnownModule {
    Rtld(kw::nnrtld),
    Main(kw::main),
    Skyline(kw::skyline),
    Sdk(kw::nnSdk),
}

impl Spanned for KnownModule {
    fn span(&self) -> proc_macro2::Span {
        match self {
            Self::Rtld(nnrtld) => nnrtld.span(),
            Self::Main(main) => main.span(),
            Self::Skyline(skyline) => skyline.span(),
            Self::Sdk(nnsdk) => nnsdk.span()
        }
    }
}

impl Parse for KnownModule {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if let Ok(nnrtld @ kw::nnrtld { .. }) = input.parse() {
            Ok(Self::Rtld(nnrtld))
        } else if let Ok(main @ kw::main { .. }) = input.parse() {
            Ok(Self::Main(main))
        } else if let Ok(skyline @ kw::skyline { .. }) = input.parse() {
            Ok(Self::Skyline(skyline))
        } else if let Ok(nnsdk @ kw::nnSdk { .. }) = input.parse() {
            Ok(Self::Sdk(nnsdk))
        } else {
            Err(syn::Error::new(input.span(), "unknown module"))
        }
    }
}

#[derive(Clone)]
pub enum ModuleArg {
    ByKnown(KnownModule),
    ByName(syn::LitStr),
}

impl Spanned for ModuleArg {
    fn span(&self) -> proc_macro2::Span {
        match self {
            Self::ByKnown(known) => known.span(),
            Self::ByName(name) => name.span()
        }
    }
}

impl Parse for ModuleArg {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if let Ok(str) = input.parse() {
            Ok(Self::ByName(str))
        } else {
            input.parse().map(Self::ByKnown)
        }
    }
}

pub enum HookStyle {
    Symbol,
    Offset,
}

impl Parse for HookStyle {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if let Ok(kw::replace { .. }) = input.parse() {
            Ok(Self::Symbol)
        } else if let Ok(kw::offset { .. }) = input.parse() {
            Ok(Self::Offset)
        } else {
            Err(syn::Error::new(input.span(), "unknown hook type"))
        }
    }
}

pub struct HookAttributes {
    pub module: Option<KeyValue<kw::module, ModuleArg>>,
    pub style: KeyValue<HookStyle, syn::Expr>,
    pub force_jit: Option<kw::force_jit>
}

impl Parse for HookAttributes {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let module = if input.peek(kw::module) {
            let m = input.parse()?;
            let _: syn::Token![,] = input.parse()?;
            Some(m)
        } else {
            None
        };

        let style = input.parse()?;

        let force_jit = if input.parse::<syn::Token![,]>().is_ok() {
            Some(input.parse::<kw::force_jit>()?)
        } else {
            None
        };

        Ok(Self { module, style, force_jit })
    }
}

pub type MainAttrs = KeyValue<kw::name, syn::LitStr>;