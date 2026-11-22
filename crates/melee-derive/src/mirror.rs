//! `#[derive(Mirror)]`: a hand-written game struct's mirror (`melee_sim::game::mirror`), from
//! the addresses its fields' attributes give.
//!
//! ```ignore
//! #[derive(melee_derive::Mirror)]
//! #[mirror(c = "Ground", size = 0x204, lay, pad(0x12, 2))]
//! pub struct Ground {
//!     #[at(0x0)]
//!     #[doc(alias("x0"))]
//!     pub unread_0: i32,
//!     #[at(0x10, bits(5, 1))]
//!     pub shadowed: bool,
//!     #[at(0x344, union)]
//!     pub script_interest: [u8; 12],
//! }
//! ```
//!
//! - `#[mirror(..)]` on the struct: `c` the decomp's name of the struct it mirrors (none for a
//!   group of fields that has no C struct of its own), `size` its bytes, `lay` to lay it out for
//!   the heap's image too (`LayOut`), and a `pad(OFFSET, BYTES)` for each run of bytes no field
//!   holds (the compiler's padding), which nothing compares.
//! - `#[at(OFFSET)]` on each field, in increasing order: a value of a `Mirror` type at that
//!   offset; `#[at(OFFSET, bits(FIRST, COUNT))]` a bitfield of the byte at OFFSET (bits counted
//!   from the top, as the compiler allocates them) of a `BitField` type; `#[at(OFFSET, union)]`
//!   a C union kept as its bytes (`[u8; N]`), compared and laid out whole.
//!
//! It implements `Mirror` (import, compare, and the layout the layout check reads) and, with
//! `lay`, `LayOut`. The comparer labels a field `path.name`, with the decomp's name beside it
//! where the field's `#[doc(alias(..))]` gives one (`path.weight (x50)`), as the generated
//! mirrors do. A const check fails the build when a field runs past the struct or into the
//! next one.

use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::{Data, DeriveInput, Fields, LitInt, LitStr, Token, parse::ParseStream};

struct StructOpts {
    c: Option<String>,
    size: u32,
    lay: bool,
    pads: Vec<(u32, u32)>,
}

enum Kind {
    Value,
    Bits(u32, u32),
    Union,
}

struct FieldOpts {
    ident: syn::Ident,
    ty: syn::Type,
    at: u32,
    kind: Kind,
    label: String,
}

fn int(input: ParseStream) -> syn::Result<u32> {
    input.parse::<LitInt>()?.base10_parse()
}

fn struct_opts(input: &DeriveInput) -> syn::Result<StructOpts> {
    let mut o = StructOpts {
        c: None,
        size: 0,
        lay: false,
        pads: Vec::new(),
    };
    let mut sized = false;
    for a in input.attrs.iter().filter(|a| a.path().is_ident("mirror")) {
        a.parse_nested_meta(|meta| {
            if meta.path.is_ident("c") {
                o.c = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("size") {
                o.size = int(meta.value()?)?;
                sized = true;
            } else if meta.path.is_ident("lay") {
                o.lay = true;
            } else if meta.path.is_ident("pad") {
                let content;
                syn::parenthesized!(content in meta.input);
                let at = int(&content)?;
                content.parse::<Token![,]>()?;
                let n = int(&content)?;
                o.pads.push((at, n));
            } else {
                return Err(meta.error("expected `c`, `size`, `lay` or `pad`"));
            }
            Ok(())
        })?;
    }
    if !sized {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "Mirror needs #[mirror(size = ..)]",
        ));
    }
    Ok(o)
}

/// The decomp's name a field's `#[doc(alias("x50"))]` (or `alias = "x50"`) gives, if any.
fn alias(field: &syn::Field) -> Option<String> {
    for a in field.attrs.iter().filter(|a| a.path().is_ident("doc")) {
        let mut found = None;
        let _ = a.parse_nested_meta(|meta| {
            if meta.path.is_ident("alias") {
                if meta.input.peek(Token![=]) {
                    found = Some(meta.value()?.parse::<LitStr>()?.value());
                } else {
                    let content;
                    syn::parenthesized!(content in meta.input);
                    found = Some(content.parse::<LitStr>()?.value());
                }
            }
            Ok(())
        });
        if found.is_some() {
            return found;
        }
    }
    None
}

fn field_opts(field: &syn::Field) -> syn::Result<FieldOpts> {
    let ident = field.ident.clone().unwrap();
    let attr = field
        .attrs
        .iter()
        .find(|a| a.path().is_ident("at"))
        .ok_or_else(|| {
            syn::Error::new_spanned(&ident, "Mirror needs #[at(OFFSET)] on every field")
        })?;
    let (at, kind) = attr.parse_args_with(|input: ParseStream| {
        let at = int(input)?;
        let mut kind = Kind::Value;
        if input.parse::<Option<Token![,]>>()?.is_some() {
            let what: syn::Ident = input.parse()?;
            if what == "bits" {
                let content;
                syn::parenthesized!(content in input);
                let first = int(&content)?;
                content.parse::<Token![,]>()?;
                let count = int(&content)?;
                if first + count > 32 || count == 0 {
                    return Err(syn::Error::new_spanned(&what, "a bitfield of 1 to 32 bits"));
                }
                kind = Kind::Bits(first, count);
            } else if what == "union" {
                kind = Kind::Union;
            } else {
                return Err(syn::Error::new_spanned(
                    what,
                    "expected `bits(FIRST, COUNT)` or `union`",
                ));
            }
        }
        Ok((at, kind))
    })?;
    let name = ident.to_string();
    let name = name.strip_prefix("r#").unwrap_or(&name).to_owned();
    let label = match alias(field) {
        Some(a) if a != name => format!("{name} ({a})"),
        _ => name,
    };
    Ok(FieldOpts {
        ident,
        ty: field.ty.clone(),
        at,
        kind,
        label,
    })
}

pub fn derive(input: DeriveInput) -> syn::Result<Tokens> {
    let opts = struct_opts(&input)?;
    let name = &input.ident;
    let Data::Struct(s) = &input.data else {
        return Err(syn::Error::new_spanned(name, "Mirror derives for structs"));
    };
    let Fields::Named(named) = &s.fields else {
        return Err(syn::Error::new_spanned(name, "Mirror needs named fields"));
    };
    let fields = named
        .named
        .iter()
        .map(field_opts)
        .collect::<syn::Result<Vec<_>>>()?;
    let m = quote!(::melee_sim::game::mirror);
    let size = opts.size;

    let mut imports = Vec::new();
    let mut compares = Vec::new();
    let mut lays = Vec::new();
    let mut layouts = Vec::new();
    // Each field's first bit and its end, as const expressions, for the build's check.
    let mut spans = Vec::new();
    for f in &fields {
        let (ident, ty, at, label) = (&f.ident, &f.ty, f.at, &f.label);
        let fname = ident.to_string();
        let path = quote!(&::std::format!("{}.{}", path, #label));
        match f.kind {
            Kind::Value => {
                imports.push(quote!(#ident: <#ty as #m::Mirror>::import(m, at + #at, cx)));
                compares.push(quote!(#m::Mirror::compare(&self.#ident, m, at + #at, #path, cx);));
                lays.push(quote!(#m::LayOut::lay(&self.#ident, l, at + #at, cx);));
                layouts.push(quote!(#m::FieldLayout {
                    name: #fname,
                    at: #at,
                    size: <#ty as #m::Mirror>::SIZE,
                    bits: None,
                    nested: <#ty as #m::Mirror>::layout,
                }));
                spans.push((
                    quote!(#at * 8),
                    quote!((#at + <#ty as #m::Mirror>::SIZE) * 8),
                    fname,
                ));
            }
            Kind::Bits(first, count) => {
                imports.push(quote!(#ident: <#ty as #m::BitField>::from_bits(m.bits(at + #at, #first, #count))));
                compares.push(quote! {
                    {
                        let theirs = m.bits(at + #at, #first, #count);
                        if #m::BitField::bits(&self.#ident) != theirs {
                            cx.mismatch(
                                #path,
                                ::std::format!("{:?}", self.#ident),
                                ::std::format!("{:?}", <#ty as #m::BitField>::from_bits(theirs)),
                            );
                        }
                    }
                });
                lays.push(
                    quote!(l.put_bits(at + #at, #first, #count, #m::BitField::bits(&self.#ident));),
                );
                layouts.push(quote!(#m::FieldLayout {
                    name: #fname,
                    at: #at,
                    size: 0,
                    bits: Some((#first, #count)),
                    nested: || None,
                }));
                spans.push((
                    quote!(#at * 8 + #first),
                    quote!(#at * 8 + #first + #count),
                    fname,
                ));
            }
            Kind::Union => {
                imports.push(quote!(#ident: m.bytes(at + #at)));
                compares.push(quote!(#m::compare_union(&self.#ident, m, at + #at, #path, cx);));
                lays.push(quote!(l.put_bytes(at + #at, &self.#ident);));
                layouts.push(quote!(#m::FieldLayout {
                    name: #fname,
                    at: #at,
                    size: ::core::mem::size_of::<#ty>() as u32,
                    bits: None,
                    nested: || None,
                }));
                spans.push((
                    quote!(#at * 8),
                    quote!((#at + ::core::mem::size_of::<#ty>() as u32) * 8),
                    fname,
                ));
            }
        }
    }
    let c = match &opts.c {
        Some(c) => quote!(Some(#c)),
        None => quote!(None),
    };
    let pads = opts.pads.iter().map(|(a, n)| quote!((#a, #n)));
    let checks = spans.iter().enumerate().map(|(i, (start, end, fname))| {
        let past = format!("{name}.{fname} runs past the struct's size");
        let into = format!(
            "{name}.{fname} starts before the field above it ends: overlapping, or out of order"
        );
        let prev = if i > 0 {
            let (_, pend, _) = &spans[i - 1];
            quote!(assert!(#pend <= #start, #into);)
        } else {
            quote!()
        };
        quote! {
            assert!(#end <= #size * 8, #past);
            #prev
        }
    });
    let lay = opts.lay.then(|| {
        quote! {
            impl #m::LayOut for #name {
                fn lay(&self, l: &mut ::melee_sim::hsd::export::Lay, at: u32, cx: &dyn #m::LayCx) {
                    let _ = (&l, at, &cx);
                    #(#lays)*
                }
            }
        }
    });
    Ok(quote! {
        impl #m::Mirror for #name {
            const SIZE: u32 = #size;

            fn import(m: &#m::Mem, at: u32, cx: &mut dyn #m::Cx) -> Self {
                let _ = (&m, at, &cx);
                #name { #(#imports),* }
            }

            fn compare(&self, m: &#m::Mem, at: u32, path: &str, cx: &mut dyn #m::Cx) {
                let _ = (&m, at, path, &cx);
                #(#compares)*
            }

            fn layout() -> Option<#m::Layout> {
                Some(#m::Layout {
                    c_name: #c,
                    size: #size,
                    pads: ::std::vec![#(#pads),*],
                    fields: ::std::vec![#(#layouts),*],
                })
            }
        }

        #lay

        #[allow(clippy::int_plus_one)]
        const _: () = {
            #(#checks)*
        };
    })
}
