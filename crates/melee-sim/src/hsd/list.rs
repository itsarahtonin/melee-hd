//! HAL's singly linked lists (`sysdolphin/baselib/list.c`), with nodes from a pool.

use super::pool::Pool;
use crate::hsd::heap::Kind;
use crate::hsd::pool::kind as pool_kind;

/// A list node's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SListId(pub u32);

/// `HSD_SList`.
#[derive(Clone, Copy, Debug)]
pub struct SList<T> {
    pub next: Option<SListId>,
    pub data: T,
}

/// The node pool (`slist_alloc_data`).
pub type SLists<T> = Pool<SList<T>, { pool_kind(Kind::SList) }>;

/// A node holding `data`, `HSD_SListAlloc`.
#[doc(alias = "HSD_SListAlloc")]
pub fn alloc<T>(nodes: &mut SLists<T>, data: T) -> SListId {
    SListId(nodes.alloc(SList { next: None, data }))
}

/// Links `node` right after `list`'s head, or makes it a list of its own, and returns the
/// head, `HSD_SListAppendList`.
#[doc(alias = "HSD_SListAppendList")]
pub fn append_list<T>(nodes: &mut SLists<T>, list: Option<SListId>, node: SListId) -> SListId {
    match list {
        Some(l) => {
            nodes[node.0].next = nodes[l.0].next;
            nodes[l.0].next = Some(node);
            l
        }
        None => {
            nodes[node.0].next = None;
            node
        }
    }
}

/// Puts `node` in front of `list`, `HSD_SListPrependList`.
#[doc(alias = "HSD_SListPrependList")]
pub fn prepend_list<T>(nodes: &mut SLists<T>, list: Option<SListId>, node: SListId) -> SListId {
    nodes[node.0].next = list;
    node
}

/// `HSD_SListAllocAndAppend`.
#[doc(alias = "HSD_SListAllocAndAppend")]
pub fn alloc_and_append<T>(nodes: &mut SLists<T>, list: Option<SListId>, data: T) -> SListId {
    let n = alloc(nodes, data);
    append_list(nodes, list, n)
}

/// `HSD_SListAllocAndPrepend`.
#[doc(alias = "HSD_SListAllocAndPrepend")]
pub fn alloc_and_prepend<T>(nodes: &mut SLists<T>, list: Option<SListId>, data: T) -> SListId {
    let n = alloc(nodes, data);
    prepend_list(nodes, list, n)
}

/// Frees the head and returns the rest, `HSD_SListRemove`.
#[doc(alias = "HSD_SListRemove")]
pub fn remove<T>(nodes: &mut SLists<T>, list: Option<SListId>) -> Option<SListId> {
    let l = list?;
    let next = nodes[l.0].next;
    nodes.free(l.0);
    next
}
