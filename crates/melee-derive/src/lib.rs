//! `#[derive(CloneFrom)]`: `Clone` whose `clone_from` copies field by field, each field's
//! own `clone_from`, so a rollback restore (`Game::clone_from`) reuses the target's vectors,
//! boxes and maps instead of building the state anew. The derived `Clone`'s `clone_from`
//! replaces the whole value. An enum copies field by field when both values are the same
//! variant, and replaces the value otherwise.
//!
//! A field marked `#[clone_from(if_changed)]` is copied only when it differs, as a whole: for
//! containers whose own `clone_from` builds anew (maps, vectors of tuples holding strings) and
//! that rarely change. A field marked `#[clone_from(keep)]` isn't state but spare storage: a
//! clone starts it empty (`Default`), and `clone_from` leaves the target's.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Index, parse_macro_input, parse_quote};

#[proc_macro_derive(CloneFrom, attributes(clone_from))]
pub fn derive_clone_from(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident,
        mut generics,
        data,
        ..
    } = parse_macro_input!(input as DeriveInput);
    let name = &ident;
    for p in generics.type_params_mut() {
        p.bounds.push(parse_quote!(::core::clone::Clone));
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let (clone, clone_from) = match &data {
        Data::Struct(s) => {
            let (ctor, from) = fields(quote!(#name), &s.fields, quote!(self.), quote!(source.));
            (ctor, from)
        }
        Data::Enum(e) => {
            let mut clone_arms = Vec::new();
            let mut from_arms = Vec::new();
            for v in &e.variants {
                let vname = &v.ident;
                let path = quote!(#name::#vname);
                match &v.fields {
                    Fields::Named(f) => {
                        let names: Vec<_> =
                            f.named.iter().map(|f| f.ident.clone().unwrap()).collect();
                        let theirs: Vec<_> = names
                            .iter()
                            .map(|n| format_ident!("source_{}", n))
                            .collect();
                        clone_arms.push(quote! {
                            #path { #(#names),* } => #path { #(#names: ::core::clone::Clone::clone(#names)),* }
                        });
                        from_arms.push(quote! {
                            (#path { #(#names),* }, #path { #(#names: #theirs),* }) => {
                                #(::core::clone::Clone::clone_from(#names, #theirs);)*
                            }
                        });
                    }
                    Fields::Unnamed(f) => {
                        let ours: Vec<_> = (0..f.unnamed.len())
                            .map(|i| format_ident!("ours_{}", i))
                            .collect();
                        let theirs: Vec<_> = (0..f.unnamed.len())
                            .map(|i| format_ident!("theirs_{}", i))
                            .collect();
                        clone_arms.push(quote! {
                            #path ( #(#ours),* ) => #path ( #(::core::clone::Clone::clone(#ours)),* )
                        });
                        from_arms.push(quote! {
                            (#path ( #(#ours),* ), #path ( #(#theirs),* )) => {
                                #(::core::clone::Clone::clone_from(#ours, #theirs);)*
                            }
                        });
                    }
                    Fields::Unit => {
                        clone_arms.push(quote!(#path => #path));
                        from_arms.push(quote!((#path, #path) => {}));
                    }
                }
            }
            let clone = quote! {
                match self { #(#clone_arms,)* }
            };
            let from = quote! {
                #[allow(unreachable_patterns)]
                match (self, source) {
                    #(#from_arms)*
                    (ours, theirs) => *ours = ::core::clone::Clone::clone(theirs),
                }
            };
            (clone, from)
        }
        Data::Union(_) => {
            return syn::Error::new_spanned(name, "CloneFrom doesn't derive for unions")
                .to_compile_error()
                .into();
        }
    };
    quote! {
        impl #impl_generics ::core::clone::Clone for #name #ty_generics #where_clause {
            #[inline]
            fn clone(&self) -> Self {
                #clone
            }
            #[inline]
            fn clone_from(&mut self, source: &Self) {
                #clone_from
            }
        }
    }
    .into()
}

/// A struct's constructor from `ours`'s fields cloned, and its fields' `clone_from`s.
fn fields(ctor: Tokens, f: &Fields, ours: Tokens, theirs: Tokens) -> (Tokens, Tokens) {
    match f {
        Fields::Named(f) => {
            let clones = f.named.iter().map(|field| {
                let n = field.ident.clone().unwrap();
                if option(field, "keep") {
                    quote!(#n: ::core::default::Default::default())
                } else {
                    quote!(#n: ::core::clone::Clone::clone(&#ours #n))
                }
            });
            let froms = f.named.iter().map(|field| {
                let n = field.ident.clone().unwrap();
                if option(field, "keep") {
                    quote!()
                } else if option(field, "if_changed") {
                    quote! {
                        if #ours #n != #theirs #n {
                            #ours #n = ::core::clone::Clone::clone(&#theirs #n);
                        }
                    }
                } else {
                    quote!(::core::clone::Clone::clone_from(&mut #ours #n, &#theirs #n);)
                }
            });
            (quote!(#ctor { #(#clones),* }), quote!(#(#froms)*))
        }
        Fields::Unnamed(f) => {
            let idx: Vec<Index> = (0..f.unnamed.len()).map(Index::from).collect();
            (
                quote!(#ctor ( #(::core::clone::Clone::clone(&#ours #idx)),* )),
                quote!(#(::core::clone::Clone::clone_from(&mut #ours #idx, &#theirs #idx);)*),
            )
        }
        Fields::Unit => (quote!(#ctor), quote!()),
    }
}

/// Whether a field is marked `#[clone_from(NAME)]`.
fn option(field: &syn::Field, name: &str) -> bool {
    field.attrs.iter().any(|a| {
        a.path().is_ident("clone_from") && a.parse_args::<syn::Ident>().is_ok_and(|i| i == name)
    })
}
