//! `#[callbacks(KIND)]`: a dispatch function written as a `match` on the original's address,
//! turned into the entries of `melee_sim::game::code`'s callback table.
//!
//! The function keeps the shape the ports wrote it in: its parameters are the callback's (the
//! address one a `u32` or a `CodeRef`), and its body matches the address, each arm a callback's
//! port. The attribute replaces it with a function of the same name that takes the table's
//! builder (`code::Entries<KIND>`) and adds one entry per address, each entry the arm's body as
//! a function of the same parameters. What the attribute reads:
//!
//! - Leading `use` and `let` statements, copied into each entry that names what they bind.
//! - The `match` on the address: a statement followed by `true` (each arm runs the callback),
//!   or the tail as `Some(match ..)` (each arm is the value), or the tail as `match ..` with
//!   arms that give the function's `Option` (each arm's `Some` is the value).
//! - Arms: literal or constant addresses joined by `|`; `x if LIST.contains(&x)`; `x if x == E`;
//!   `x if other(params..)` with an empty body, and a last arm (`_` or a binding) that is
//!   `false`, `None`, or another dispatch function called with the same parameters, all of
//!   whose entries join this table (a delegation).
//! - A body that is only delegations: `a(..) || b(..)` or `a(..).or_else(|| b(..))`.
//!
//! Anything else is a compile error, so a dispatch that depends on more than the address can't
//! slip into the table.

use proc_macro2::{Group, Ident, Span, TokenStream as Tokens, TokenTree};
use quote::{ToTokens, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Arm, BinOp, Block, Expr, FnArg, ItemFn, Pat, ReturnType, Stmt, Type, parse_macro_input};

#[proc_macro_attribute]
pub fn callbacks(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let kind = parse_macro_input!(attr as Ident);
    let f = parse_macro_input!(item as ItemFn);
    match expand(&kind, &f) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

type Res<T> = Result<T, syn::Error>;

fn err<T>(s: impl Spanned, msg: &str) -> Res<T> {
    Err(syn::Error::new(s.span(), format!("#[callbacks]: {msg}")))
}

struct Sig {
    /// Each parameter's name as the entries take it (`self` as `__self`).
    params: Vec<Ident>,
    /// The address parameter's index.
    addr: usize,
    /// Whether the address parameter is a `CodeRef` (matched as `x.0`).
    code_ref: bool,
    method: bool,
    /// The function returns an `Option`.
    option: bool,
}

fn signature(f: &ItemFn) -> Res<Sig> {
    let mut params = Vec::new();
    let mut addr = None;
    let mut code_ref = false;
    let mut method = false;
    for (i, a) in f.sig.inputs.iter().enumerate() {
        match a {
            FnArg::Receiver(_) => {
                method = true;
                params.push(Ident::new("__self", Span::call_site()));
            }
            FnArg::Typed(t) => {
                let Pat::Ident(p) = &*t.pat else {
                    return err(&t.pat, "a parameter that isn't a name");
                };
                params.push(p.ident.clone());
                if let Type::Path(tp) = &*t.ty {
                    let last = tp.path.segments.last().unwrap().ident.to_string();
                    if last == "u32" || last == "CodeRef" {
                        if addr.is_some() {
                            return err(&t.ty, "two address parameters");
                        }
                        addr = Some(i);
                        code_ref = last == "CodeRef";
                    }
                }
            }
        }
    }
    let Some(addr) = addr else {
        return err(&f.sig, "no address parameter (`u32` or `CodeRef`)");
    };
    let option = match &f.sig.output {
        ReturnType::Type(_, t) => t.to_token_stream().to_string().starts_with("Option"),
        ReturnType::Default => return err(&f.sig, "the function returns nothing"),
    };
    Ok(Sig {
        params,
        addr,
        code_ref,
        method,
        option,
    })
}

/// How an arm's body becomes an entry's value.
#[derive(Clone)]
enum Value {
    /// The arm runs the callback, then the entry gives this (`()` when empty).
    Then(Tokens),
    /// The arm's value is the entry's.
    Plain,
    /// The arm gives `Some(value)`.
    Some,
}

struct Gen<'a> {
    sig: &'a Sig,
    kind: &'a Ident,
    preamble: Vec<Stmt>,
    out: Vec<Tokens>,
}

fn expand(kind: &Ident, f: &ItemFn) -> Res<Tokens> {
    let sig = signature(f)?;
    let mut g = Gen {
        sig: &sig,
        kind,
        preamble: Vec::new(),
        out: Vec::new(),
    };
    g.body(&f.block)?;
    let vis = &f.vis;
    let name = &f.sig.ident;
    let attrs = f.attrs.iter().filter(|a| !a.path().is_ident("inline"));
    let out = &g.out;
    let uses = g.preamble.iter().filter(|s| matches!(s, Stmt::Item(_)));
    // The function as written, as a type: what its signature names stays in use.
    let tys = f.sig.inputs.iter().map(|a| match a {
        FnArg::Receiver(_) => quote!(&mut Self),
        FnArg::Typed(t) => t.ty.to_token_stream(),
    });
    let output = &f.sig.output;
    Ok(quote! {
        #(#attrs)*
        #vis fn #name(__t: &mut crate::game::code::Entries<crate::game::code::kind::#kind>) {
            let _: fn(#(#tys),*) #output;
            #(#[allow(unused_imports)] #uses)*
            #(#out)*
        }
    })
}

impl Gen<'_> {
    fn body(&mut self, block: &Block) -> Res<()> {
        let mut stmts = block.stmts.as_slice();
        while let Some((s, rest)) = stmts.split_first() {
            match s {
                Stmt::Item(syn::Item::Use(_)) | Stmt::Local(_) if !rest.is_empty() => {
                    self.preamble.push(s.clone());
                    stmts = rest;
                }
                Stmt::Expr(Expr::If(i), _) if !rest.is_empty() => {
                    self.leading_if(i)?;
                    stmts = rest;
                }
                _ => break,
            }
        }
        match stmts {
            // `match addr { .. }` then `true`.
            [
                Stmt::Expr(Expr::Match(m), _),
                Stmt::Expr(Expr::Lit(l), None),
            ] if !self.sig.option && l.to_token_stream().to_string() == "true" => {
                self.scrutinee(&m.expr)?;
                self.arms(&m.arms, &Value::Then(Tokens::new()))
            }
            // `match addr { .. }` then `Some(value)`.
            [
                Stmt::Expr(Expr::Match(m), _),
                Stmt::Expr(Expr::Call(c), None),
            ] if self.sig.option && is_path(&c.func, "Some") && c.args.len() == 1 => {
                self.scrutinee(&m.expr)?;
                let v = c.args[0].to_token_stream();
                self.arms(&m.arms, &Value::Then(v))
            }
            [Stmt::Expr(tail, None)] => self.tail(tail),
            _ => err(block, "a body this attribute doesn't read"),
        }
    }

    /// `if addr == X { ..; return v; }` (an entry), or `if let Some(v) = a(..) { return Some(v); }`
    /// (delegations).
    fn leading_if(&mut self, i: &syn::ExprIf) -> Res<()> {
        if i.else_branch.is_some() {
            return err(i, "a leading `if` with an `else`");
        }
        let ret = match i.then_branch.stmts.last() {
            Some(Stmt::Expr(Expr::Return(r), _)) => r.expr.as_deref(),
            _ => return err(i, "a leading `if` that doesn't end in `return`"),
        };
        let Some(ret) = ret else {
            return err(i, "a bare `return`");
        };
        if let Expr::Let(l) = &*i.cond {
            // `if let Some(v) = chain { return Some(v); }`
            let ok = flat(&l.pat) == "Some(v)"
                && flat(ret) == "Some(v)"
                && i.then_branch.stmts.len() == 1;
            if ok && self.delegations(&l.expr, None)? {
                return Ok(());
            }
            return err(i, "an `if let` that isn't a delegation");
        }
        let Expr::Binary(bin) = &*i.cond else {
            return err(&i.cond, "a leading `if` that doesn't test the address");
        };
        let addr = self.sig.params[self.sig.addr].to_string();
        let a = if !matches!(bin.op, BinOp::Eq(_)) {
            return err(&i.cond, "a leading `if` that doesn't test the address");
        } else if flat(&bin.left) == addr {
            &bin.right
        } else if flat(&bin.right) == addr {
            &bin.left
        } else {
            return err(&i.cond, "a leading `if` that doesn't test the address");
        };
        let value = if self.sig.option {
            match ret {
                Expr::Call(c) if is_path(&c.func, "Some") && c.args.len() == 1 => {
                    Value::Then(c.args[0].to_token_stream())
                }
                _ => return err(ret, "a leading `if` returning other than `Some(..)`"),
            }
        } else if flat(ret) == "true" {
            Value::Then(Tokens::new())
        } else {
            return err(ret, "a leading `if` returning other than `true`");
        };
        let n = i.then_branch.stmts.len() - 1;
        let stmts = &i.then_branch.stmts[..n];
        let body: Expr = syn::parse_quote!({ #(#stmts)* });
        let entry = self.entry(&body, &value, None);
        let site = quote_spanned! {i.cond.span()=> concat!(file!(), ":", line!()) };
        self.out
            .push(quote_spanned! {i.cond.span()=> __t.add(#a, #entry, #site); });
        Ok(())
    }

    fn tail(&mut self, tail: &Expr) -> Res<()> {
        let t = flat(tail);
        if t == "None" || t == "false" {
            return Ok(());
        }
        match tail {
            Expr::Call(c) if self.sig.option && is_path(&c.func, "Some") && c.args.len() == 1 => {
                match &c.args[0] {
                    Expr::Match(m) => {
                        self.scrutinee(&m.expr)?;
                        self.arms(&m.arms, &Value::Plain)
                    }
                    other => err(other, "`Some(..)` around something other than a match"),
                }
            }
            Expr::Match(m) if self.sig.option => {
                self.scrutinee(&m.expr)?;
                self.arms(&m.arms, &Value::Some)
            }
            other => {
                if self.delegations(other, None)? {
                    Ok(())
                } else {
                    err(other, "a tail this attribute doesn't read")
                }
            }
        }
    }

    fn scrutinee(&self, e: &Expr) -> Res<()> {
        let want = if self.sig.code_ref {
            format!("{} . 0", self.sig.params[self.sig.addr])
        } else {
            self.sig.params[self.sig.addr].to_string()
        };
        if flat(e) == want.replace(' ', "") {
            Ok(())
        } else {
            err(e, &format!("the match isn't on the address (`{want}`)"))
        }
    }

    fn arms(&mut self, arms: &[Arm], value: &Value) -> Res<()> {
        for (i, arm) in arms.iter().enumerate() {
            let last = i + 1 == arms.len();
            self.arm(arm, value, last)?;
        }
        Ok(())
    }

    fn arm(&mut self, arm: &Arm, value: &Value, last: bool) -> Res<()> {
        // A binding (`x`, `_`, `x if ..`).
        let binding = match &arm.pat {
            Pat::Wild(_) => Some(None),
            Pat::Ident(p)
                if p.subpat.is_none()
                    && p.ident
                        .to_string()
                        .starts_with(|c: char| c.is_ascii_lowercase()) =>
            {
                Some(Some(p.ident.clone()))
            }
            _ => None,
        };
        if let Some(b) = binding {
            let Some((_, guard)) = &arm.guard else {
                if !last {
                    return err(arm, "a catch-all arm before the last");
                }
                return self.fallback(&arm.body, b.as_ref());
            };
            let Some(b) = b else {
                return err(guard, "a guard on `_`");
            };
            return self.guard(arm, &b, guard, value);
        }
        let mut addrs = Vec::new();
        addresses(&arm.pat, &mut addrs)?;
        if let Some((_, g)) = &arm.guard {
            // A range's words: `lo..=hi if x.is_multiple_of(4)`.
            let word = format!("{}.is_multiple_of(4)", self.sig.params[self.sig.addr]);
            if flat(g) != word || !addrs.iter().all(|a| a.1) {
                return err(g, "a guard that depends on more than the address");
            }
        }
        let entry = self.entry(&arm.body, value, None);
        let site = site(arm);
        let kind = self.kind;
        let mut adds = Vec::new();
        for (a, range) in addrs {
            adds.push(if range {
                quote_spanned! {arm.pat.span()=> for __a in #a { __t.add(__a, __f, #site); } }
            } else {
                quote_spanned! {arm.pat.span()=> __t.add(#a, __f, #site); }
            });
        }
        self.out.push(quote_spanned! {arm.pat.span()=> {
            let __f: <crate::game::code::kind::#kind as crate::game::code::kind::Kind>::Fn = #entry;
            #(#adds)*
        }});
        Ok(())
    }

    /// `x if ..`.
    fn guard(&mut self, arm: &Arm, b: &Ident, guard: &Expr, value: &Value) -> Res<()> {
        let site = site(arm);
        // `x if LIST.contains(&x)`.
        if let Expr::MethodCall(mc) = guard
            && mc.method == "contains"
            && mc.args.len() == 1
            && mc.args[0].to_token_stream().to_string() == format!("& {b}")
        {
            let list = &mc.receiver;
            let entry = self.entry(&arm.body, value, Some(b));
            self.out.push(quote_spanned! {arm.pat.span()=>
                for &__a in #list.iter() { __t.add(__a, #entry, #site); }
            });
            return Ok(());
        }
        // `x if x == E`, `x if E == x`.
        if let Expr::Binary(bin) = guard
            && matches!(bin.op, BinOp::Eq(_))
        {
            let (l, r) = (&bin.left, &bin.right);
            let other = if *b == flat(l) {
                r
            } else if *b == flat(r) {
                l
            } else {
                return err(guard, "an equality that doesn't test the address");
            };
            let entry = self.entry(&arm.body, value, Some(b));
            self.out
                .push(quote_spanned! {arm.pat.span()=> __t.add(#other, #entry, #site); });
            return Ok(());
        }
        // `x if other(params..)`: a delegation, with an empty body.
        if self.delegations(guard, Some(b))? {
            let empty = match &*arm.body {
                Expr::Block(bl) => bl.block.stmts.is_empty(),
                Expr::Tuple(t) => t.elems.is_empty(),
                _ => false,
            };
            if !empty {
                return err(&arm.body, "a delegation's arm with a body");
            }
            return Ok(());
        }
        err(guard, "a guard this attribute doesn't read")
    }

    /// The last arm: nothing more, or a delegation.
    fn fallback(&mut self, body: &Expr, b: Option<&Ident>) -> Res<()> {
        let e = strip_return(body);
        let s = e.to_token_stream().to_string();
        if s == "false" || s == "None" {
            return Ok(());
        }
        if self.delegations(e, b)? {
            return Ok(());
        }
        err(
            body,
            "a last arm that isn't `false`, `None` or a delegation",
        )
    }

    /// Whether `e` is one or more delegations (`a(..) || b(..)`, `a(..).or_else(|| b(..))`),
    /// each added.
    fn delegations(&mut self, e: &Expr, b: Option<&Ident>) -> Res<bool> {
        let mut calls = Vec::new();
        if !split_chain(e, &mut calls) {
            return Ok(false);
        }
        let mut out = Vec::new();
        for c in calls {
            match self.delegation(c, b) {
                Some(t) => out.push(t),
                None => return Ok(false),
            }
        }
        self.out.extend(out);
        Ok(true)
    }

    /// A call of another dispatch function with this one's parameters, as its entries added.
    fn delegation(&self, e: &Expr, b: Option<&Ident>) -> Option<Tokens> {
        let names: Vec<String> = self
            .sig
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| match b {
                Some(b) if i == self.sig.addr => b.to_string(),
                _ => p.to_string(),
            })
            .collect();
        let arg_ok = |a: &Expr, want: &str| a.to_token_stream().to_string() == want;
        match e {
            Expr::Call(c) => {
                let Expr::Path(p) = &*c.func else { return None };
                if c.args.len() != names.len() {
                    return None;
                }
                for (a, want) in c.args.iter().zip(&names) {
                    let want = if self.sig.method && want == "__self" {
                        "self"
                    } else {
                        want
                    };
                    if !arg_ok(a, want) {
                        return None;
                    }
                }
                Some(quote_spanned! {e.span()=> #p(__t); })
            }
            Expr::MethodCall(mc) => {
                // `self.m(rest..)` or `param.m(rest..)` with the receiver the first parameter.
                let recv = mc.receiver.to_token_stream().to_string();
                let first = if self.sig.method {
                    "self".to_string()
                } else {
                    names[0].clone()
                };
                if recv != first || mc.args.len() + 1 != names.len() {
                    return None;
                }
                for (a, want) in mc.args.iter().zip(&names[1..]) {
                    if !arg_ok(a, want) {
                        return None;
                    }
                }
                let m = &mc.method;
                if self.sig.method {
                    Some(quote_spanned! {e.span()=> Self::#m(__t); })
                } else {
                    Some(quote_spanned! {e.span()=> crate::game::state::Running::#m(__t); })
                }
            }
            _ => None,
        }
    }

    /// An entry: the arm's body as a function of the parameters, with the preamble it names.
    fn entry(&self, body: &Expr, value: &Value, b: Option<&Ident>) -> Tokens {
        let mut body_tokens = body.to_token_stream();
        // An entry takes the address as a `u32` (`__addr`); a `CodeRef` parameter is made
        // from it, and a binding is it.
        let addr = &self.sig.params[self.sig.addr];
        let raw = if self.sig.code_ref {
            Ident::new("__addr", Span::call_site())
        } else {
            addr.clone()
        };
        if let Some(b) = b
            && idents(&body_tokens).contains(&b.to_string())
        {
            body_tokens = quote!({ let #b: u32 = #raw; #body_tokens });
        }
        if self.sig.code_ref && idents(&body_tokens).contains(&addr.to_string()) {
            body_tokens = quote!({ let #addr = crate::game::handles::CodeRef(#raw); #body_tokens });
        }
        let body_tokens = rename_self(body_tokens);
        // The preamble statements the body names, and what they name in turn.
        let mut used: Vec<String> = idents(&body_tokens);
        if let Value::Then(t) = value {
            used.extend(idents(&rename_self(t.clone())));
        }
        let mut keep = vec![false; self.preamble.len()];
        for (i, s) in self.preamble.iter().enumerate().rev() {
            match s {
                Stmt::Item(_) => {}
                Stmt::Local(l) => {
                    let bound = idents(&l.pat.to_token_stream());
                    if bound.iter().any(|x| used.contains(x)) {
                        keep[i] = true;
                        used.extend(idents(&rename_self(s.to_token_stream())));
                    }
                }
                _ => {}
            }
        }
        let pre: Vec<Tokens> = self
            .preamble
            .iter()
            .zip(&keep)
            .filter(|&(_, &k)| k)
            .map(|(s, _)| match s {
                Stmt::Item(i) => quote!(#[allow(unused_imports)] #i),
                s => rename_self(s.to_token_stream()),
            })
            .collect();
        let all: Vec<String> = {
            let mut v = used.clone();
            for p in &pre {
                v.extend(idents(p));
            }
            v
        };
        let params: Vec<Tokens> = self
            .sig
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let p = if i == self.sig.addr { &raw } else { p };
                if all.contains(&p.to_string()) {
                    quote!(#p)
                } else {
                    quote!(_)
                }
            })
            .collect();
        let value = match value {
            Value::Then(t) => {
                let t = rename_self(t.clone());
                quote!({ #(#pre)* #body_tokens; #t })
            }
            Value::Plain => quote!({ #(#pre)* #body_tokens }),
            Value::Some => {
                let kind = self.kind.to_string();
                quote!({ #(#pre)* ::core::option::Option::expect(#body_tokens, #kind) })
            }
        };
        quote_spanned! {body.span()=> |#(#params),*| #value }
    }
}

fn site(arm: &Arm) -> Tokens {
    quote_spanned! {arm.pat.span()=> concat!(file!(), ":", line!()) }
}

/// `e`'s tokens without spaces.
fn flat(e: &impl ToTokens) -> String {
    e.to_token_stream().to_string().replace(' ', "")
}

fn is_path(e: &Expr, name: &str) -> bool {
    matches!(e, Expr::Path(p) if p.path.is_ident(name))
}

fn strip_return(e: &Expr) -> &Expr {
    match e {
        Expr::Return(r) => r.expr.as_deref().map_or(e, strip_return),
        Expr::Block(b) if b.block.stmts.len() == 1 => match &b.block.stmts[0] {
            Stmt::Expr(e, _) => strip_return(e),
            _ => e,
        },
        Expr::Paren(p) => strip_return(&p.expr),
        _ => e,
    }
}

/// `a || b || c` or `a.or_else(|| b).or_else(|| c)`, as its calls.
fn split_chain<'e>(e: &'e Expr, out: &mut Vec<&'e Expr>) -> bool {
    match strip_return(e) {
        Expr::Binary(b) if matches!(b.op, BinOp::Or(_)) => {
            split_chain(&b.left, out) && split_chain(&b.right, out)
        }
        Expr::MethodCall(mc) if mc.method == "or_else" && mc.args.len() == 1 => {
            let Expr::Closure(c) = &mc.args[0] else {
                return false;
            };
            c.inputs.is_empty() && split_chain(&mc.receiver, out) && split_chain(&c.body, out)
        }
        e @ (Expr::Call(_) | Expr::MethodCall(_)) => {
            out.push(e);
            true
        }
        _ => false,
    }
}

/// An arm's addresses: literals and constants joined by `|`.
fn addresses(p: &Pat, out: &mut Vec<(Tokens, bool)>) -> Res<()> {
    match p {
        Pat::Or(o) => {
            for c in &o.cases {
                addresses(c, out)?;
            }
            Ok(())
        }
        Pat::Lit(l) => {
            out.push((l.to_token_stream(), false));
            Ok(())
        }
        Pat::Path(pp) => {
            out.push((pp.to_token_stream(), false));
            Ok(())
        }
        Pat::Ident(i) if i.subpat.is_none() => {
            out.push((i.ident.to_token_stream(), false));
            Ok(())
        }
        // Every word in `lo..=hi` (functions are word-aligned).
        Pat::Range(r) => {
            let (Some(lo), Some(hi)) = (&r.start, &r.end) else {
                return err(r, "an open range");
            };
            if !matches!(r.limits, syn::RangeLimits::Closed(_)) {
                return err(r, "a range that isn't `..=`");
            }
            out.push((quote!((#lo..=#hi).step_by(4)), true));
            Ok(())
        }
        Pat::Paren(pp) => addresses(&pp.pat, out),
        other => err(other, "an address pattern that isn't literals or constants"),
    }
}

/// Every identifier in `t`.
fn idents(t: &Tokens) -> Vec<String> {
    let mut out = Vec::new();
    for tt in t.clone() {
        match tt {
            TokenTree::Ident(i) => out.push(i.to_string()),
            TokenTree::Group(g) => out.extend(idents(&g.stream())),
            _ => {}
        }
    }
    out
}

/// `self` (not `self::`) as `__self`, in the entries of a method.
fn rename_self(t: Tokens) -> Tokens {
    let tts: Vec<TokenTree> = t.into_iter().collect();
    let mut out = Vec::with_capacity(tts.len());
    for (i, tt) in tts.iter().enumerate() {
        match tt {
            TokenTree::Ident(id) if id == "self" => {
                let path =
                    matches!(tts.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == ':');
                if path {
                    out.push(tt.clone());
                } else {
                    out.push(TokenTree::Ident(Ident::new("__self", id.span())));
                }
            }
            TokenTree::Group(g) => {
                let mut ng = Group::new(g.delimiter(), rename_self(g.stream()));
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
            }
            _ => out.push(tt.clone()),
        }
    }
    out.into_iter().collect()
}
