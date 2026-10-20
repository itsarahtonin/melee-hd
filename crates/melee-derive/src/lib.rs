//! `#[derive(CloneFrom)]`: `Clone` whose `clone_from` copies field by field, each field's
//! own `clone_from`, so a rollback restore (`Game::clone_from`) reuses the target's vectors,
//! boxes and maps instead of building the state anew. The derived `Clone`'s `clone_from`
//! replaces the whole value. An enum copies field by field when both values are the same
//! variant, and replaces the value otherwise.
//!
//! `#[derive(StateEq)]`: `melee_sim::state_eq::StateEq`, a bitwise comparison that names the
//! first field path where two values differ, for the sync test, or every one
//! (`state_diffs`). It compares every field
//! except those marked `#[clone_from(keep)]` (spare storage, not state) or
//! `#[state_eq(skip)]`.
//!
//! A field marked `#[clone_from(if_changed)]` is copied only when it differs bit for bit
//! (`melee_sim::state_eq::StateEq`), as a whole: for containers whose own `clone_from` builds
//! anew (maps, vectors of tuples holding strings) and that rarely change. A field marked
//! `#[clone_from(keep)]` isn't state but spare storage: a clone starts it empty (`Default`),
//! and `clone_from` leaves the target's.

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
                    // Bit for bit: `PartialEq` would take 0.0 for -0.0 and leave the target's.
                    quote! {
                        if ::melee_sim::state_eq::StateEq::state_diff(&#ours #n, &#theirs #n).is_some() {
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

#[proc_macro_derive(StateEq, attributes(clone_from, state_eq))]
pub fn derive_state_eq(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident,
        generics,
        data,
        ..
    } = parse_macro_input!(input as DeriveInput);
    let name = &ident;
    let trait_path = quote!(::melee_sim::state_eq::StateEq);
    let diff = quote!(::melee_sim::state_eq::Diff);
    let params: Vec<String> = generics
        .type_params()
        .map(|p| p.ident.to_string())
        .collect();
    // Each compared field's type that names a type parameter must compare too.
    let mut bounds: Vec<syn::Type> = Vec::new();
    let mut bound = |ty: &syn::Type| {
        if mentions(quote!(#ty), &params) && !bounds.contains(ty) {
            bounds.push(ty.clone());
        }
    };
    let nest = quote!(::melee_sim::state_eq::nest);
    // Each field's diffs, nested in it: one call a field, since a function with many fields'
    // loops inline (a fighter's) overflows LLVM's stack.
    let collect = |o: Tokens, t: Tokens, label: &str| {
        let segment = format!(".{label}");
        quote!(::melee_sim::state_eq::diffs_in(#o, #t, out, #segment);)
    };
    let (body, all) = match &data {
        Data::Struct(s) => {
            let mut all = Vec::new();
            let checks = s.fields.iter().enumerate().filter(|(_, f)| compared(f)).map(|(i, f)| {
                bound(&f.ty);
                let (access, label) = match &f.ident {
                    Some(n) => (quote!(#n), n.to_string()),
                    None => {
                        let i = Index::from(i);
                        (quote!(#i), i.index.to_string())
                    }
                };
                all.push(collect(quote!(&self.#access), quote!(&other.#access), &label));
                quote! {
                    if let ::core::option::Option::Some(d) = #trait_path::state_diff(&self.#access, &other.#access) {
                        return ::core::option::Option::Some(d.field(#label));
                    }
                }
            }).collect::<Vec<_>>();
            (
                quote! { #(#checks)* ::core::option::Option::None },
                quote! { #(#all)* },
            )
        }
        Data::Enum(e) => {
            let mut arms = Vec::new();
            let mut all_arms = Vec::new();
            let mut names = Vec::new();
            for v in &e.variants {
                let vname = &v.ident;
                let label = vname.to_string();
                let segment = format!("::{label}");
                let path = quote!(#name::#vname);
                match &v.fields {
                    Fields::Named(f) => {
                        let all: Vec<_> =
                            f.named.iter().map(|f| f.ident.clone().unwrap()).collect();
                        let ours: Vec<_> =
                            all.iter().map(|n| format_ident!("ours_{}", n)).collect();
                        let theirs: Vec<_> =
                            all.iter().map(|n| format_ident!("theirs_{}", n)).collect();
                        let mut collects = Vec::new();
                        let checks = f.named.iter().zip(ours.iter().zip(&theirs)).filter(|(f, _)| compared(f)).map(|(f, (o, t))| {
                            bound(&f.ty);
                            let fl = f.ident.as_ref().unwrap().to_string();
                            collects.push(collect(quote!(#o), quote!(#t), &fl));
                            quote! {
                                if let ::core::option::Option::Some(d) = #trait_path::state_diff(#o, #t) {
                                    return ::core::option::Option::Some(d.field(#fl).variant(#label));
                                }
                            }
                        }).collect::<Vec<_>>();
                        arms.push(quote! {
                            (#path { #(#all: #ours),* }, #path { #(#all: #theirs),* }) => {
                                #(#checks)*
                                ::core::option::Option::None
                            }
                        });
                        all_arms.push(quote! {
                            (#path { #(#all: #ours),* }, #path { #(#all: #theirs),* }) => {
                                let start = out.len();
                                #(#collects)*
                                #nest(out, start, #segment);
                            }
                        });
                        names.push(quote!(#path { .. } => #label));
                    }
                    Fields::Unnamed(f) => {
                        let ours: Vec<_> = (0..f.unnamed.len())
                            .map(|i| format_ident!("ours_{}", i))
                            .collect();
                        let theirs: Vec<_> = (0..f.unnamed.len())
                            .map(|i| format_ident!("theirs_{}", i))
                            .collect();
                        let mut collects = Vec::new();
                        let checks = f.unnamed.iter().enumerate().zip(ours.iter().zip(&theirs)).filter(|((_, f), _)| compared(f)).map(|((i, f), (o, t))| {
                            bound(&f.ty);
                            let fl = i.to_string();
                            collects.push(collect(quote!(#o), quote!(#t), &fl));
                            quote! {
                                if let ::core::option::Option::Some(d) = #trait_path::state_diff(#o, #t) {
                                    return ::core::option::Option::Some(d.field(#fl).variant(#label));
                                }
                            }
                        }).collect::<Vec<_>>();
                        arms.push(quote! {
                            (#path ( #(#ours),* ), #path ( #(#theirs),* )) => {
                                #(#checks)*
                                ::core::option::Option::None
                            }
                        });
                        all_arms.push(quote! {
                            (#path ( #(#ours),* ), #path ( #(#theirs),* )) => {
                                let start = out.len();
                                #(#collects)*
                                #nest(out, start, #segment);
                            }
                        });
                        names.push(quote!(#path ( .. ) => #label));
                    }
                    Fields::Unit => {
                        arms.push(quote!((#path, #path) => ::core::option::Option::None,));
                        all_arms.push(quote!((#path, #path) => {}));
                        names.push(quote!(#path => #label));
                    }
                }
            }
            let (mismatch, all_mismatch) = if e.variants.len() > 1 {
                (
                    quote! {
                        (ours, theirs) => {
                            let variant = |v: &Self| -> &'static str { match v { #(#names,)* } };
                            ::core::option::Option::Some(#diff::new(::std::format!(
                                "variant {} != {}", variant(ours), variant(theirs)
                            )))
                        }
                    },
                    quote! {
                        _ => out.extend(#trait_path::state_diff(self, other)),
                    },
                )
            } else {
                (quote!(), quote!())
            };
            (
                quote! {
                    #[allow(unreachable_patterns, unused_variables)]
                    match (self, other) {
                        #(#arms)*
                        #mismatch
                    }
                },
                quote! {
                    #[allow(unreachable_patterns, unused_variables)]
                    match (self, other) {
                        #(#all_arms)*
                        #all_mismatch
                    }
                },
            )
        }
        Data::Union(_) => {
            return syn::Error::new_spanned(name, "StateEq doesn't derive for unions")
                .to_compile_error()
                .into();
        }
    };
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut where_clause = where_clause.cloned().unwrap_or_else(|| parse_quote!(where));
    for ty in &bounds {
        where_clause.predicates.push(parse_quote!(#ty: #trait_path));
    }
    quote! {
        impl #impl_generics #trait_path for #name #ty_generics #where_clause {
            #[inline]
            fn state_diff(&self, other: &Self) -> ::core::option::Option<#diff> {
                #body
            }
            #[allow(unused_variables)]
            fn state_diffs(&self, other: &Self, out: &mut ::std::vec::Vec<#diff>) {
                #all
            }
        }
    }
    .into()
}

/// Whether the sync test compares a field: not spare storage (`#[clone_from(keep)]`) nor
/// marked `#[state_eq(skip)]`.
fn compared(field: &syn::Field) -> bool {
    !option(field, "keep")
        && !field.attrs.iter().any(|a| {
            a.path().is_ident("state_eq") && a.parse_args::<syn::Ident>().is_ok_and(|i| i == "skip")
        })
}

/// Whether `tokens` name one of `params`.
fn mentions(tokens: Tokens, params: &[String]) -> bool {
    tokens.into_iter().any(|t| match t {
        proc_macro2::TokenTree::Ident(i) => params.iter().any(|p| i == p),
        proc_macro2::TokenTree::Group(g) => mentions(g.stream(), params),
        _ => false,
    })
}
