//! The source of the crates a leaf-check binary links, cut into units by item: each function
//! (an impl's or trait's methods each on their own), each impl's and trait's shell (its header
//! and everything but its methods' bodies), each other item. A unit has a hash of its tokens
//! (comments and doc attributes left out: neither changes the program), the lines it spans, the
//! names it defines and the identifiers it mentions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use syn::spanned::Spanned;

use crate::hash::hex;

/// What a unit is, which decides how [`crate::reach`] takes it in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A function with a body: what the coverage pass sees run.
    Fn,
    /// A `const fn`, which also runs at compile time, where coverage can't see it.
    ConstFn,
    /// An impl's or trait's header with its associated constants and types (and a trait's
    /// method signatures).
    Shell,
    /// A struct, enum, union, type alias, const, static, extern crate or foreign block.
    Named,
    /// A `macro_rules!` definition, named by its name and everything it mentions.
    MacroDef,
    /// A macro invoked where items go, named by every identifier in it but the macro's (it may
    /// define them).
    MacroCall,
    /// A private `use`: it decides what the names it imports mean in its own file. Its names
    /// are what it calls its imports (`*` for a glob), its identifiers the imported items'.
    Use,
    /// A public `use`: a name other modules reach through it.
    PubUse,
    /// A whole file that didn't parse.
    File,
}

#[derive(Debug)]
pub struct Unit {
    /// `FILE|MODULES|CONTAINER|KIND NAME#N`, unique in the index.
    pub key: String,
    pub file: usize,
    pub kind: Kind,
    /// The names it defines, by which other units' identifiers find it.
    pub names: Vec<String>,
    /// First and last line (from 1).
    pub lines: (usize, usize),
    pub hash: String,
    /// A function's signature with its attributes and its impl's or trait's header.
    pub sig: Option<String>,
    /// The identifiers it mentions (a method's include its impl's header).
    pub idents: Vec<String>,
    /// A method's impl or trait shell.
    pub container: Option<usize>,
}

#[derive(Debug)]
pub struct SourceFile {
    /// Relative to the repository, with `/`.
    pub rel: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct Index {
    pub files: Vec<SourceFile>,
    pub units: Vec<Unit>,
    pub by_name: HashMap<String, Vec<usize>>,
    pub by_key: HashMap<String, usize>,
    pub by_rel: HashMap<String, usize>,
    /// Each file's units.
    pub in_file: Vec<Vec<usize>>,
}

impl Index {
    /// Indexes the files `rels` (relative to `root`).
    pub fn build(root: &Path, rels: &[String]) -> Index {
        let mut index = Index::default();
        for rel in rels {
            let bytes = std::fs::read(root.join(rel))
                .unwrap_or_else(|e| panic!("{}: {e}", root.join(rel).display()));
            let f = index.files.len();
            index.by_rel.insert(rel.clone(), f);
            index.in_file.push(Vec::new());
            let text = String::from_utf8_lossy(&bytes).into_owned();
            index.files.push(SourceFile {
                rel: rel.clone(),
                bytes,
            });
            let mut units = Vec::new();
            match syn::parse_file(&text) {
                Ok(file) => {
                    let mut w = Walker {
                        rel,
                        file: f,
                        modules: Vec::new(),
                        units: &mut units,
                    };
                    w.items(&file.items);
                }
                Err(_) => units.push(Unit {
                    key: format!("{rel}|||file"),
                    file: f,
                    kind: Kind::File,
                    names: Vec::new(),
                    lines: (1, text.lines().count().max(1)),
                    hash: hex(text.as_bytes()),
                    sig: None,
                    idents: text
                        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    container: None,
                }),
            }
            let base = index.units.len();
            let mut seen: HashMap<String, usize> = HashMap::new();
            for mut u in units {
                let n = seen.entry(u.key.clone()).or_default();
                u.key = format!("{}#{n}", u.key);
                *n += 1;
                u.container = u.container.map(|c| c + base);
                let i = index.units.len();
                // A private use names nothing outside its file ([`crate::reach::closure`]).
                if u.kind != Kind::Use {
                    for name in &u.names {
                        index.by_name.entry(name.clone()).or_default().push(i);
                    }
                }
                index.by_key.insert(u.key.clone(), i);
                index.in_file[f].push(i);
                index.units.push(u);
            }
        }
        index
    }

    /// The innermost unit of file `f` holding line `line`, if any.
    pub fn unit_at(&self, f: usize, line: usize) -> Option<usize> {
        self.in_file[f]
            .iter()
            .copied()
            .filter(|&u| (self.units[u].lines.0..=self.units[u].lines.1).contains(&line))
            .min_by_key(|&u| self.units[u].lines.1 - self.units[u].lines.0)
    }
}

struct Walker<'a> {
    rel: &'a str,
    file: usize,
    modules: Vec<String>,
    units: &'a mut Vec<Unit>,
}

impl Walker<'_> {
    fn key(&self, container: &str, what: &str) -> String {
        format!(
            "{}|{}|{container}|{what}",
            self.rel,
            self.modules.join("::")
        )
    }

    fn push(&mut self, mut u: Unit) -> usize {
        u.file = self.file;
        self.units.push(u);
        self.units.len() - 1
    }

    fn unit(&self, key: String, kind: Kind, names: Vec<String>, node: &dyn ToTokens) -> Unit {
        let tokens = docless(node.to_token_stream());
        Unit {
            key,
            file: self.file,
            kind,
            names,
            lines: lines(node),
            hash: hex(tokens.to_string().as_bytes()),
            sig: None,
            idents: idents(&tokens),
            container: None,
        }
    }

    fn items(&mut self, items: &[syn::Item]) {
        for item in items {
            self.item(item);
        }
    }

    fn item(&mut self, item: &syn::Item) {
        use syn::Item;
        match item {
            Item::Fn(f) => {
                let name = f.sig.ident.to_string();
                let kind = if f.sig.constness.is_some() {
                    Kind::ConstFn
                } else {
                    Kind::Fn
                };
                let mut u = self.unit(self.key("", &format!("fn {name}")), kind, vec![name], f);
                u.sig = Some(sig_hash(&f.attrs, &f.sig, None));
                self.push(u);
            }
            Item::Impl(i) => {
                let header = impl_header(i);
                let mut shell = i.clone();
                shell.items.retain(|it| !matches!(it, syn::ImplItem::Fn(_)));
                let names = shell
                    .items
                    .iter()
                    .filter_map(|it| match it {
                        syn::ImplItem::Const(c) => Some(c.ident.to_string()),
                        syn::ImplItem::Type(t) => Some(t.ident.to_string()),
                        _ => None,
                    })
                    .collect();
                let mut s = self.unit(self.key(&header, "shell"), Kind::Shell, names, &shell);
                s.lines = lines(i);
                let header_idents = idents(&docless(header_tokens(i)));
                let c = self.push(s);
                for it in &i.items {
                    if let syn::ImplItem::Fn(f) = it {
                        let name = f.sig.ident.to_string();
                        let kind = if f.sig.constness.is_some() {
                            Kind::ConstFn
                        } else {
                            Kind::Fn
                        };
                        let mut u = self.unit(
                            self.key(&header, &format!("fn {name}")),
                            kind,
                            vec![name],
                            f,
                        );
                        u.sig = Some(sig_hash(&f.attrs, &f.sig, Some(&header)));
                        u.idents.extend(header_idents.iter().cloned());
                        u.container = Some(c);
                        self.push(u);
                    }
                }
            }
            Item::Trait(t) => {
                let header = format!("trait {}", t.ident);
                let mut shell = t.clone();
                for it in &mut shell.items {
                    if let syn::TraitItem::Fn(f) = it {
                        f.default = None;
                        f.semi_token = Some(Default::default());
                    }
                }
                let mut names = vec![t.ident.to_string()];
                names.extend(t.items.iter().filter_map(|it| match it {
                    syn::TraitItem::Const(c) => Some(c.ident.to_string()),
                    syn::TraitItem::Type(t) => Some(t.ident.to_string()),
                    _ => None,
                }));
                let mut s = self.unit(self.key(&header, "shell"), Kind::Shell, names, &shell);
                s.lines = lines(t);
                let c = self.push(s);
                for it in &t.items {
                    if let syn::TraitItem::Fn(f) = it
                        && f.default.is_some()
                    {
                        let name = f.sig.ident.to_string();
                        let mut u = self.unit(
                            self.key(&header, &format!("fn {name}")),
                            Kind::Fn,
                            vec![name],
                            f,
                        );
                        u.sig = Some(sig_hash(&f.attrs, &f.sig, Some(&header)));
                        u.idents.push(t.ident.to_string());
                        u.container = Some(c);
                        self.push(u);
                    }
                }
            }
            Item::Mod(m) => {
                if let Some((_, items)) = &m.content {
                    self.modules.push(m.ident.to_string());
                    self.items(items);
                    self.modules.pop();
                }
            }
            Item::Struct(s) => self.named("struct", &s.ident, vec![], item),
            // By its name alone: a variant is named through it (`Kind::Fox`, `Self::Fox`) or
            // imported (`use Kind::*`, whose path names it).
            Item::Enum(e) => self.named("enum", &e.ident, vec![], item),
            Item::Union(u) => self.named("union", &u.ident, vec![], item),
            Item::Type(t) => self.named("type", &t.ident, vec![], item),
            Item::Const(c) => self.named("const", &c.ident, vec![], item),
            Item::Static(s) => self.named("static", &s.ident, vec![], item),
            Item::ExternCrate(e) => self.named("extern crate", &e.ident, vec![], item),
            Item::TraitAlias(t) => self.named("trait alias", &t.ident, vec![], item),
            Item::Macro(m) => {
                if let Some(ident) = &m.ident {
                    // Named by everything it mentions too: what it defines may be named only
                    // in its body (an enum, a const), and code naming those may run none of it.
                    let name = ident.to_string();
                    let mut u = self.unit(
                        self.key("", &format!("macro {name}")),
                        Kind::MacroDef,
                        vec![],
                        item,
                    );
                    u.names = u.idents.clone();
                    u.names.push(name);
                    self.push(u);
                } else {
                    // Named by every identifier in it but the macro's own: it may define them.
                    let path_idents: Vec<String> = m
                        .mac
                        .path
                        .segments
                        .iter()
                        .map(|s| s.ident.to_string())
                        .collect();
                    let path = m.mac.path.to_token_stream().to_string();
                    let mut u = self.unit(
                        self.key("", &format!("{path}!")),
                        Kind::MacroCall,
                        vec![],
                        item,
                    );
                    u.names = u
                        .idents
                        .iter()
                        .filter(|i| !path_idents.contains(i))
                        .cloned()
                        .collect();
                    self.push(u);
                }
            }
            Item::Use(u) => {
                let public = !matches!(u.vis, syn::Visibility::Inherited);
                let (mut names, mut originals) = (Vec::new(), Vec::new());
                use_names(&u.tree, &mut names, &mut originals);
                let what = format!("use {}", u.tree.to_token_stream());
                let kind = if public { Kind::PubUse } else { Kind::Use };
                let mut unit = self.unit(self.key("", &what), kind, names, item);
                // What it imports and the path it takes (an enum's variants are imported
                // through the enum).
                originals.extend(path_idents(&u.tree));
                originals.sort();
                originals.dedup();
                unit.idents = originals;
                self.push(unit);
            }
            Item::ForeignMod(f) => {
                let names = f
                    .items
                    .iter()
                    .filter_map(|it| match it {
                        syn::ForeignItem::Fn(f) => Some(f.sig.ident.to_string()),
                        syn::ForeignItem::Static(s) => Some(s.ident.to_string()),
                        syn::ForeignItem::Type(t) => Some(t.ident.to_string()),
                        _ => None,
                    })
                    .collect();
                let u = self.unit(self.key("", "extern"), Kind::Named, names, item);
                self.push(u);
            }
            // `Verbatim` and anything newer: named by every identifier in it, as a macro is.
            other => {
                let mut u = self.unit(self.key("", "item"), Kind::MacroCall, vec![], other);
                u.names = u.idents.clone();
                self.push(u);
            }
        }
    }

    fn named(&mut self, what: &str, ident: &syn::Ident, more: Vec<String>, item: &syn::Item) {
        let mut names = vec![ident.to_string()];
        names.extend(more);
        let u = self.unit(
            self.key("", &format!("{what} {ident}")),
            Kind::Named,
            names,
            item,
        );
        self.push(u);
    }
}

fn lines(node: &dyn ToTokens) -> (usize, usize) {
    let mut lo = usize::MAX;
    let mut hi = 0;
    // The tokens' own spans: an item's joined span can lose its attributes' lines.
    fn walk(ts: TokenStream, lo: &mut usize, hi: &mut usize) {
        for t in ts {
            let s = t.span();
            *lo = (*lo).min(s.start().line);
            *hi = (*hi).max(s.end().line);
            if let TokenTree::Group(g) = t {
                walk(g.stream(), lo, hi);
            }
        }
    }
    walk(node.to_token_stream(), &mut lo, &mut hi);
    if lo == usize::MAX {
        let s = node.to_token_stream().span();
        return (s.start().line, s.end().line);
    }
    (lo, hi)
}

fn header_tokens(i: &syn::ItemImpl) -> TokenStream {
    let mut t = TokenStream::new();
    for a in &i.attrs {
        a.to_tokens(&mut t);
    }
    i.defaultness.to_tokens(&mut t);
    i.unsafety.to_tokens(&mut t);
    i.impl_token.to_tokens(&mut t);
    i.generics.to_tokens(&mut t);
    if let Some((bang, path, for_)) = &i.trait_ {
        bang.to_tokens(&mut t);
        path.to_tokens(&mut t);
        for_.to_tokens(&mut t);
    }
    i.self_ty.to_tokens(&mut t);
    i.generics.where_clause.to_tokens(&mut t);
    t
}

/// An impl's header as text: `impl<..> Trait for Type where ..`, attributes and docs left out.
fn impl_header(i: &syn::ItemImpl) -> String {
    let mut t = TokenStream::new();
    i.impl_token.to_tokens(&mut t);
    i.generics.to_tokens(&mut t);
    if let Some((bang, path, for_)) = &i.trait_ {
        bang.to_tokens(&mut t);
        path.to_tokens(&mut t);
        for_.to_tokens(&mut t);
    }
    i.self_ty.to_tokens(&mut t);
    i.generics.where_clause.to_tokens(&mut t);
    t.to_string()
}

fn sig_hash(attrs: &[syn::Attribute], sig: &syn::Signature, header: Option<&str>) -> String {
    let mut t = TokenStream::new();
    for a in attrs {
        a.to_tokens(&mut t);
    }
    sig.to_tokens(&mut t);
    let text = format!("{} {}", header.unwrap_or(""), docless(t));
    hex(text.as_bytes())
}

/// A use's names (what it calls each import, `*` for a glob) and the imported items' own.
fn use_names(tree: &syn::UseTree, names: &mut Vec<String>, originals: &mut Vec<String>) {
    match tree {
        syn::UseTree::Path(p) => use_names(&p.tree, names, originals),
        syn::UseTree::Name(n) => {
            names.push(n.ident.to_string());
            originals.push(n.ident.to_string());
        }
        syn::UseTree::Rename(r) => {
            names.push(r.rename.to_string());
            originals.push(r.ident.to_string());
        }
        syn::UseTree::Glob(_) => names.push("*".to_owned()),
        syn::UseTree::Group(g) => g.items.iter().for_each(|t| use_names(t, names, originals)),
    }
}

/// The path segments of a use, but the imported names.
fn path_idents(tree: &syn::UseTree) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(t: &syn::UseTree, out: &mut Vec<String>) {
        match t {
            syn::UseTree::Path(p) => {
                out.push(p.ident.to_string());
                walk(&p.tree, out);
            }
            syn::UseTree::Group(g) => g.items.iter().for_each(|t| walk(t, out)),
            _ => {}
        }
    }
    walk(tree, &mut out);
    out
}

/// Tokens without doc attributes (`#[doc ...]`, `#![doc ...]`, which `///` and `//!` are).
pub fn docless(ts: TokenStream) -> TokenStream {
    let tokens: Vec<TokenTree> = ts.into_iter().collect();
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Punct(p) = &tokens[i]
            && p.as_char() == '#'
        {
            let bang =
                matches!(&tokens.get(i + 1), Some(TokenTree::Punct(b)) if b.as_char() == '!');
            let at = i + 1 + usize::from(bang);
            if let Some(TokenTree::Group(g)) = tokens.get(at)
                && g.delimiter() == Delimiter::Bracket
                && matches!(g.stream().into_iter().next(), Some(TokenTree::Ident(id)) if id == "doc")
            {
                i = at + 1;
                continue;
            }
        }
        out.push(match &tokens[i] {
            TokenTree::Group(g) => {
                let mut ng = proc_macro2::Group::new(g.delimiter(), docless(g.stream()));
                ng.set_span(g.span());
                TokenTree::Group(ng)
            }
            t => t.clone(),
        });
        i += 1;
    }
    out.into_iter().collect()
}

/// The identifiers in `ts`, and those a string literal names in braces (`format!("{NAME}")`).
pub fn idents(ts: &TokenStream) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(ts: &TokenStream, out: &mut Vec<String>) {
        for t in ts.clone() {
            match t {
                TokenTree::Ident(i) => {
                    let s = i.to_string();
                    out.push(s.strip_prefix("r#").map(str::to_owned).unwrap_or(s));
                }
                TokenTree::Group(g) => walk(&g.stream(), out),
                TokenTree::Literal(l) => {
                    let s = l.to_string();
                    if s.contains('"') {
                        for part in s.split('{').skip(1) {
                            let name: String = part
                                .chars()
                                .take_while(|c| c.is_alphanumeric() || *c == '_')
                                .collect();
                            if !name.is_empty() && !name.starts_with(|c: char| c.is_ascii_digit()) {
                                out.push(name);
                            }
                        }
                    }
                }
                TokenTree::Punct(_) => {}
            }
        }
    }
    walk(ts, &mut out);
    out.sort();
    out.dedup();
    out
}

/// The `.rs` files under `dir` (relative to `root`), sorted, as relative paths with `/`.
pub fn rust_files(root: &Path, dir: &str, skip: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.join(dir)];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p: PathBuf = e.path();
            let rel = rel_of(root, &p);
            if p.is_dir() {
                if !skip(&rel) {
                    stack.push(p);
                }
            } else if rel.ends_with(".rs") && !skip(&rel) {
                out.push(rel);
            }
        }
    }
    out.sort();
    out
}

/// `p` relative to `root`, with `/`.
pub fn rel_of(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// A directory of its own for a test's files.
#[cfg(test)]
pub fn scratch_dir() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "leaf-select-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_of(src: &str) -> Index {
        let dir = scratch_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.rs"), src).unwrap();
        let i = Index::build(&dir, &["a.rs".to_owned()]);
        let _ = std::fs::remove_dir_all(&dir);
        i
    }

    #[test]
    fn units_by_item_with_lines() {
        let i = index_of(
            "/// Doc.\nconst A: u32 = 1;\n\nstruct S { x: u32 }\n\nimpl S {\n    const B: u32 = 2;\n    fn f(&self) -> u32 {\n        self.x + A\n    }\n}\n\nfn g<T: Copy>(t: T) -> T {\n    let s = format!(\"{A}\");\n    t\n}\n",
        );
        let by = |k: &str| &i.units[i.by_key[k]];
        let f = by("a.rs||impl S|fn f#0");
        assert_eq!(f.lines, (8, 10));
        assert_eq!(f.kind, Kind::Fn);
        assert!(f.idents.contains(&"A".to_owned()) && f.idents.contains(&"S".to_owned()));
        let shell = by("a.rs||impl S|shell#0");
        assert_eq!(shell.names, vec!["B".to_owned()]);
        assert_eq!(f.container, Some(i.by_key["a.rs||impl S|shell#0"]));
        assert_eq!(i.unit_at(0, 9), Some(i.by_key["a.rs||impl S|fn f#0"]));
        assert_eq!(i.unit_at(0, 7), Some(i.by_key["a.rs||impl S|shell#0"]));
        assert_eq!(i.unit_at(0, 3), None);
        assert_eq!(by("a.rs|||const A#0").lines, (1, 2));
        // A literal's braces name what it formats.
        assert!(by("a.rs|||fn g#0").idents.contains(&"A".to_owned()));
    }

    #[test]
    fn comments_and_docs_dont_change_a_hash() {
        let a = index_of("fn f() -> u32 {\n    1\n}\n");
        let b =
            index_of("/// Says one.\n#[doc = \"more\"]\nfn f() -> u32 {\n    // one\n    1\n}\n");
        let c = index_of("fn f() -> u32 {\n    2\n}\n");
        let h = |i: &Index| i.units[i.by_key["a.rs|||fn f#0"]].hash.clone();
        assert_eq!(h(&a), h(&b));
        assert_ne!(h(&a), h(&c));
    }
}
