//! Slab DOM that implements Stylo's `TElement` / `TNode`.
//! The engine behind the runner is real Stylo; this is only the host tree.

#![allow(unsafe_code)]

use std::cell::Cell;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::os::raw::c_void;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::matching::{ElementSelectorFlags, MatchingContext};
use selectors::sink::Push;
use selectors::OpaqueElement;
use selectors::{Element as SelectorsElement, SelectorImpl};
use style::applicable_declarations::ApplicableDeclarationBlock;
use style::context::{QuirksMode, SharedStyleContext};
use style::data::{ElementData, ElementDataMut, ElementDataRef, ElementDataWrapper};
use style::dom::{
    LayoutIterator, NodeInfo, OpaqueNode, TDocument, TElement, TNode, TShadowRoot,
};
use style::invalidation::element::restyle_hints::RestyleHint;
use style::properties::{parse_style_attribute, PropertyDeclarationBlock};
use style::selector_parser::{AttrValue, Lang, PseudoElement, RestyleDamage};
use style::shared_lock::{Locked, SharedRwLock};
use style::stylesheets::{CssRuleType, UrlExtraData};
use style::stylist::CascadeData;
use style::values::computed::Display;
use style::values::{AtomIdent, AtomString};
use style::{Atom, LocalName, Namespace};
use style::servo_arc::{Arc, ArcBorrow};
use stylebench_fixture::{Fixture, Mut};
use url::Url;
use web_atoms::{ns, LocalName as WebLocalName, Namespace as WebNs};

fn parse_inline_style(
    attrs: &[(WebLocalName, String)],
    lock: &SharedRwLock,
) -> Option<Arc<Locked<PropertyDeclarationBlock>>> {
    let style_name = WebLocalName::from("style");
    let value = attrs.iter().find(|(n, _)| n == &style_name)?.1.as_str();
    if value.is_empty() {
        return None;
    }
    let url = UrlExtraData(Arc::new(Url::parse("about:blank").unwrap()));
    let pdb = parse_style_attribute(
        value,
        &url,
        None,
        QuirksMode::NoQuirks,
        CssRuleType::Style,
    );
    Some(Arc::new(lock.wrap(pdb)))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoShadow;

impl TShadowRoot for NoShadow {
    type ConcreteNode = Node;

    fn as_node(&self) -> Node {
        unreachable!("no shadow DOM in the StyleBench host")
    }
    fn host(&self) -> Elem {
        unreachable!("no shadow DOM")
    }
    fn style_data<'a>(&self) -> Option<&'a CascadeData>
    where
        Self: 'a,
    {
        None
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Document,
    Element,
}

struct Slot {
    doc: *const HostDoc,
    id: u32,
    kind: Kind,
    parent: Option<u32>,
    first_child: Option<u32>,
    last_child: Option<u32>,
    next: Option<u32>,
    prev: Option<u32>,
    tag: WebLocalName,
    ns: WebNs,
    dom_id: Option<Atom>,
    classes: Vec<AtomIdent>,
    attrs: Vec<(WebLocalName, String)>,
    style_attr: Option<Arc<Locked<PropertyDeclarationBlock>>>,
    data: ElementDataWrapper,
    dirty_descendants: AtomicBool,
    children_to_process: AtomicIsize,
    selector_flags: Cell<ElementSelectorFlags>,
    handled_snapshot: Cell<bool>,
    dead: Cell<bool>,
}

pub struct HostDoc {
    slots: Vec<Slot>,
    lock: SharedRwLock,
}

impl HostDoc {
    pub fn from_fixture(fix: &Fixture) -> Box<Self> {
        let html_ns: WebNs = ns!(html);
        let lock = SharedRwLock::new();
        let mut slots = Vec::with_capacity(fix.nodes.len() + 1 + fix.leaf_adds());
        slots.push(Slot {
            doc: std::ptr::null(),
            id: 0,
            kind: Kind::Document,
            parent: None,
            first_child: None,
            last_child: None,
            next: None,
            prev: None,
            tag: WebLocalName::from("#document"),
            ns: html_ns.clone(),
            dom_id: None,
            classes: Vec::new(),
            attrs: Vec::new(),
            style_attr: None,
            data: ElementDataWrapper::default(),
            dirty_descendants: AtomicBool::new(false),
            children_to_process: AtomicIsize::new(0),
            selector_flags: Cell::new(ElementSelectorFlags::empty()),
            handled_snapshot: Cell::new(true),
            dead: Cell::new(false),
        });
        for n in &fix.nodes {
            let parent = if n.parent < 0 {
                0
            } else {
                (n.parent as u32) + 1
            };
            let data = ElementDataWrapper::default();
            data.borrow_mut().hint = RestyleHint::restyle_subtree();
            slots.push(Slot {
                doc: std::ptr::null(),
                id: slots.len() as u32,
                kind: Kind::Element,
                parent: Some(parent),
                first_child: None,
                last_child: None,
                next: None,
                prev: None,
                tag: WebLocalName::from(&*n.tag),
                ns: html_ns.clone(),
                dom_id: n.dom_id.as_ref().map(|s| Atom::from(&**s)),
                classes: n.classes.iter().map(|c| AtomIdent::from(&**c)).collect(),
                attrs: n
                    .attrs
                    .iter()
                    .map(|a| (WebLocalName::from(&*a.name), a.value.clone()))
                    .collect(),
                style_attr: None,
                data,
                dirty_descendants: AtomicBool::new(true),
                children_to_process: AtomicIsize::new(0),
                selector_flags: Cell::new(ElementSelectorFlags::empty()),
                handled_snapshot: Cell::new(true),
                dead: Cell::new(false),
            });
        }
        for i in 1..slots.len() {
            let p = slots[i].parent.expect("element parent") as usize;
            if let Some(last) = slots[p].last_child {
                slots[i].prev = Some(last);
                slots[last as usize].next = Some(i as u32);
            } else {
                slots[p].first_child = Some(i as u32);
            }
            slots[p].last_child = Some(i as u32);
        }
        for s in &mut slots {
            s.style_attr = parse_inline_style(&s.attrs, &lock);
        }
        let mut boxed = Box::new(HostDoc { slots, lock });
        let doc_ptr = &*boxed as *const HostDoc;
        for s in &mut boxed.slots {
            s.doc = doc_ptr;
        }
        boxed
    }

    pub fn lock(&self) -> &SharedRwLock {
        &self.lock
    }

    pub fn root_elem(&self) -> Elem {
        Elem(Node(&self.slots[1]))
    }

    pub fn each_element(&self, mut f: impl FnMut(Elem)) {
        for id in 1..self.slots.len() {
            if self.slots[id].dead.get() {
                continue;
            }
            f(Elem(Node(&self.slots[id])));
        }
    }

    pub fn live_count(&self) -> usize {
        self.slots[1..].iter().filter(|s| !s.dead.get()).count()
    }

    fn fixture_slot(&mut self, id: i32) -> Option<&mut Slot> {
        let i = (id as u32).checked_add(1)? as usize;
        self.slots.get_mut(i)
    }

    pub fn dirty_all(&self) {
        for s in &self.slots[1..] {
            if s.dead.get() {
                continue;
            }
            s.data.borrow_mut().hint = RestyleHint::restyle_subtree();
            s.dirty_descendants.store(true, Ordering::Relaxed);
        }
    }

    pub fn apply_mut(&mut self, m: &Mut) {
        match m {
            Mut::AddClass { id, class } => {
                if let Some(s) = self.fixture_slot(*id) {
                    let atom = AtomIdent::from(&**class);
                    if !s.classes.iter().any(|c| c == &atom) {
                        s.classes.push(atom);
                    }
                }
            }
            Mut::RemoveClass { id } => {
                if let Some(s) = self.fixture_slot(*id) {
                    if !s.classes.is_empty() {
                        s.classes.remove(0);
                    }
                }
            }
            Mut::SetAttr { id, name, value } => {
                if let Some(s) = self.fixture_slot(*id) {
                    let ln = WebLocalName::from(&**name);
                    if let Some(a) = s.attrs.iter_mut().find(|(n, _)| n == &ln) {
                        a.1 = value.clone();
                    } else {
                        s.attrs.push((ln, value.clone()));
                    }
                }
                if let Some(i) = (*id as u32).checked_add(1).map(|x| x as usize) {
                    if i < self.slots.len() {
                        let pdb = parse_inline_style(&self.slots[i].attrs, &self.lock);
                        self.slots[i].style_attr = pdb;
                    }
                }
            }
            Mut::RemoveAttr { id, name } => {
                if let Some(s) = self.fixture_slot(*id) {
                    let ln = WebLocalName::from(&**name);
                    s.attrs.retain(|(n, _)| n != &ln);
                }
                if let Some(i) = (*id as u32).checked_add(1).map(|x| x as usize) {
                    if i < self.slots.len() {
                        let pdb = parse_inline_style(&self.slots[i].attrs, &self.lock);
                        self.slots[i].style_attr = pdb;
                    }
                }
            }
            Mut::AddLeaf {
                id,
                parent,
                at,
                tag,
                dom_id,
                classes,
                attrs,
            } => {
                self.add_leaf(*id, *parent, *at, tag, dom_id.as_deref(), classes, attrs);
            }
            Mut::RemoveLeaf { id } => self.remove_leaf(*id),
            Mut::Restyle => {}
        }
    }

    fn add_leaf(
        &mut self,
        id: i32,
        parent: i32,
        at: i32,
        tag: &str,
        dom_id: Option<&str>,
        classes: &[String],
        attrs: &[stylebench_fixture::Attr],
    ) {
        let html_ns: WebNs = ns!(html);
        let parent_slot = (parent as u32) + 1;
        let new_id = (id as u32) + 1;
        if new_id as usize != self.slots.len() {
            return;
        }
        let data = ElementDataWrapper::default();
        data.borrow_mut().hint = RestyleHint::restyle_subtree();
        self.slots.push(Slot {
            doc: std::ptr::null(),
            id: new_id,
            kind: Kind::Element,
            parent: Some(parent_slot),
            first_child: None,
            last_child: None,
            next: None,
            prev: None,
            tag: WebLocalName::from(tag),
            ns: html_ns,
            dom_id: dom_id.map(Atom::from),
            classes: classes.iter().map(|c| AtomIdent::from(&**c)).collect(),
            attrs: attrs
                .iter()
                .map(|a| (WebLocalName::from(&*a.name), a.value.clone()))
                .collect(),
            style_attr: None,
            data,
            dirty_descendants: AtomicBool::new(true),
            children_to_process: AtomicIsize::new(0),
            selector_flags: Cell::new(ElementSelectorFlags::empty()),
            handled_snapshot: Cell::new(true),
            dead: Cell::new(false),
        });
        let doc_ptr = &*self as *const HostDoc;
        self.slots[new_id as usize].doc = doc_ptr;
        let pdb = parse_inline_style(&self.slots[new_id as usize].attrs, &self.lock);
        self.slots[new_id as usize].style_attr = pdb;
        self.insert_child(parent_slot, at as usize, new_id);
    }

    fn live_kids(&self, parent: u32) -> Vec<u32> {
        let mut kids = Vec::new();
        let mut n = self.slots[parent as usize].first_child;
        while let Some(id) = n {
            if !self.slots[id as usize].dead.get() {
                kids.push(id);
            }
            n = self.slots[id as usize].next;
        }
        kids
    }

    fn insert_child(&mut self, parent: u32, at: usize, child: u32) {
        let kids = self.live_kids(parent);
        if let Some(&before) = kids.get(at) {
            let prev = self.slots[before as usize].prev;
            self.slots[child as usize].parent = Some(parent);
            self.slots[child as usize].next = Some(before);
            self.slots[child as usize].prev = prev;
            self.slots[before as usize].prev = Some(child);
            if let Some(p) = prev {
                self.slots[p as usize].next = Some(child);
            } else {
                self.slots[parent as usize].first_child = Some(child);
            }
        } else {
            let last = self.slots[parent as usize].last_child;
            self.slots[child as usize].parent = Some(parent);
            self.slots[child as usize].prev = last;
            self.slots[child as usize].next = None;
            if let Some(l) = last {
                self.slots[l as usize].next = Some(child);
            } else {
                self.slots[parent as usize].first_child = Some(child);
            }
            self.slots[parent as usize].last_child = Some(child);
        }
    }

    fn remove_leaf(&mut self, id: i32) {
        let slot_id = (id as u32) + 1;
        if slot_id as usize >= self.slots.len() {
            return;
        }
        let parent = self.slots[slot_id as usize].parent;
        let prev = self.slots[slot_id as usize].prev;
        let next = self.slots[slot_id as usize].next;
        if let Some(p) = prev {
            self.slots[p as usize].next = next;
        } else if let Some(par) = parent {
            self.slots[par as usize].first_child = next;
        }
        if let Some(n) = next {
            self.slots[n as usize].prev = prev;
        } else if let Some(par) = parent {
            self.slots[par as usize].last_child = prev;
        }
        let s = &mut self.slots[slot_id as usize];
        s.dead.set(true);
        s.parent = None;
        s.prev = None;
        s.next = None;
        s.first_child = None;
        s.last_child = None;
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Node(*const Slot);

unsafe impl Send for Node {}
unsafe impl Sync for Node {}

impl Node {
    fn slot(&self) -> &Slot {
        unsafe { &*self.0 }
    }
    fn doc(&self) -> &HostDoc {
        unsafe { &*self.slot().doc }
    }
    fn at(&self, id: u32) -> Node {
        Node(&self.doc().slots[id as usize])
    }
    pub fn index(&self) -> u32 {
        self.slot().id
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Node({})", self.index())
    }
}

impl NodeInfo for Node {
    fn is_element(&self) -> bool {
        matches!(self.slot().kind, Kind::Element)
    }
    fn is_text_node(&self) -> bool {
        false
    }
}

impl TNode for Node {
    type ConcreteElement = Elem;
    type ConcreteDocument = Doc;
    type ConcreteShadowRoot = NoShadow;

    fn parent_node(&self) -> Option<Self> {
        self.slot().parent.map(|id| self.at(id))
    }
    fn first_child(&self) -> Option<Self> {
        self.slot().first_child.map(|id| self.at(id))
    }
    fn last_child(&self) -> Option<Self> {
        self.slot().last_child.map(|id| self.at(id))
    }
    fn prev_sibling(&self) -> Option<Self> {
        self.slot().prev.map(|id| self.at(id))
    }
    fn next_sibling(&self) -> Option<Self> {
        self.slot().next.map(|id| self.at(id))
    }
    fn owner_doc(&self) -> Doc {
        Doc(self.at(0))
    }
    fn is_in_document(&self) -> bool {
        !self.slot().dead.get()
    }
    fn traversal_parent(&self) -> Option<Elem> {
        self.parent_element()
    }
    fn opaque(&self) -> OpaqueNode {
        OpaqueNode(self.index() as usize)
    }
    fn debug_id(self) -> usize {
        self.index() as usize
    }
    fn as_element(&self) -> Option<Elem> {
        if self.is_element() {
            Some(Elem(*self))
        } else {
            None
        }
    }
    fn as_document(&self) -> Option<Doc> {
        if matches!(self.slot().kind, Kind::Document) {
            Some(Doc(*self))
        } else {
            None
        }
    }
    fn as_shadow_root(&self) -> Option<NoShadow> {
        None
    }
}

#[derive(Clone, Copy)]
pub struct Doc(Node);

impl TDocument for Doc {
    type ConcreteNode = Node;

    fn as_node(&self) -> Node {
        self.0
    }
    fn is_html_document(&self) -> bool {
        true
    }
    fn quirks_mode(&self) -> QuirksMode {
        QuirksMode::NoQuirks
    }
    fn shared_lock(&self) -> &SharedRwLock {
        self.0.doc().lock()
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Elem(pub Node);

impl PartialEq for Elem {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Elem {}

impl Hash for Elem {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.index().hash(state);
    }
}

impl fmt::Debug for Elem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "<{}#{}>",
            self.0.slot().tag,
            self.0
                .slot()
                .dom_id
                .as_ref()
                .map(|a| a.as_ref())
                .unwrap_or("")
        )
    }
}

impl Elem {
    fn slot(&self) -> &Slot {
        self.0.slot()
    }
}

impl SelectorsElement for Elem {
    type Impl = style::selector_parser::SelectorImpl;

    fn opaque(&self) -> OpaqueElement {
        // Must name the slab slot, not a Copy handle on the stack.
        OpaqueElement::new(self.0.slot())
    }
    fn parent_element(&self) -> Option<Self> {
        self.0.parent_element()
    }
    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }
    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }
    fn is_pseudo_element(&self) -> bool {
        false
    }
    fn prev_sibling_element(&self) -> Option<Self> {
        let mut n = self.0.prev_sibling();
        while let Some(node) = n {
            if let Some(e) = node.as_element() {
                return Some(e);
            }
            n = node.prev_sibling();
        }
        None
    }
    fn next_sibling_element(&self) -> Option<Self> {
        let mut n = self.0.next_sibling();
        while let Some(node) = n {
            if let Some(e) = node.as_element() {
                return Some(e);
            }
            n = node.next_sibling();
        }
        None
    }
    fn first_element_child(&self) -> Option<Self> {
        let mut n = self.0.first_child();
        while let Some(node) = n {
            if let Some(e) = node.as_element() {
                return Some(e);
            }
            n = node.next_sibling();
        }
        None
    }
    fn is_html_element_in_html_document(&self) -> bool {
        self.slot().ns == ns!(html)
    }
    fn has_local_name(&self, local_name: &WebLocalName) -> bool {
        &self.slot().tag == local_name
    }
    fn has_namespace(&self, ns: &WebNs) -> bool {
        &self.slot().ns == ns
    }
    fn is_same_type(&self, other: &Self) -> bool {
        self.slot().tag == other.slot().tag && self.slot().ns == other.slot().ns
    }
    fn attr_matches(
        &self,
        ns: &NamespaceConstraint<&<Self::Impl as SelectorImpl>::NamespaceUrl>,
        local_name: &<Self::Impl as SelectorImpl>::LocalName,
        operation: &AttrSelectorOperation<&<Self::Impl as SelectorImpl>::AttrValue>,
    ) -> bool {
        match ns {
            NamespaceConstraint::Specific(url) if !url.0.is_empty() => return false,
            _ => {}
        }
        let want: &WebLocalName = local_name;
        for (name, val) in &self.slot().attrs {
            if name == want && operation.eval_str(val) {
                return true;
            }
        }
        false
    }
    fn match_non_ts_pseudo_class(
        &self,
        _: &<Self::Impl as SelectorImpl>::NonTSPseudoClass,
        _: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        false
    }
    fn match_pseudo_element(
        &self,
        _: &<Self::Impl as SelectorImpl>::PseudoElement,
        _: &mut MatchingContext<Self::Impl>,
    ) -> bool {
        false
    }
    fn apply_selector_flags(&self, flags: ElementSelectorFlags) {
        let cur = self.slot().selector_flags.get();
        self.slot().selector_flags.set(cur | flags);
    }
    fn is_link(&self) -> bool {
        false
    }
    fn is_html_slot_element(&self) -> bool {
        false
    }
    fn has_id(&self, id: &<Self::Impl as SelectorImpl>::Identifier, case: CaseSensitivity) -> bool {
        let Some(have) = self.slot().dom_id.as_ref() else {
            return false;
        };
        match case {
            CaseSensitivity::CaseSensitive => have == &id.0,
            CaseSensitivity::AsciiCaseInsensitive => have.eq_ignore_ascii_case(&id.0),
        }
    }
    fn has_class(
        &self,
        name: &<Self::Impl as SelectorImpl>::Identifier,
        case: CaseSensitivity,
    ) -> bool {
        self.slot().classes.iter().any(|c| match case {
            CaseSensitivity::CaseSensitive => c == name,
            CaseSensitivity::AsciiCaseInsensitive => c.0.eq_ignore_ascii_case(&name.0),
        })
    }
    fn has_custom_state(&self, _: &<Self::Impl as SelectorImpl>::Identifier) -> bool {
        false
    }
    fn imported_part(
        &self,
        _: &<Self::Impl as SelectorImpl>::Identifier,
    ) -> Option<<Self::Impl as SelectorImpl>::Identifier> {
        None
    }
    fn is_part(&self, _: &<Self::Impl as SelectorImpl>::Identifier) -> bool {
        false
    }
    fn is_empty(&self) -> bool {
        self.0.first_child().is_none()
    }
    fn is_root(&self) -> bool {
        matches!(
            self.0.parent_node().map(|n| n.slot().kind),
            Some(Kind::Document)
        )
    }
    fn add_element_unique_hashes(&self, filter: &mut selectors::bloom::BloomFilter) -> bool {
        use style::bloom::each_relevant_element_hash;
        each_relevant_element_hash(*self, |hash| filter.insert_hash(hash));
        true
    }
}

pub struct Kids(Option<Node>);

impl Iterator for Kids {
    type Item = Node;
    fn next(&mut self) -> Option<Node> {
        let n = self.0.take()?;
        self.0 = n.next_sibling();
        Some(n)
    }
}

impl TElement for Elem {
    type ConcreteNode = Node;
    type TraversalChildrenIterator = Kids;

    fn as_node(&self) -> Node {
        self.0
    }
    fn traversal_children(&self) -> LayoutIterator<Kids> {
        LayoutIterator(Kids(self.0.first_child()))
    }
    fn is_html_element(&self) -> bool {
        self.slot().ns == ns!(html)
    }
    fn is_mathml_element(&self) -> bool {
        false
    }
    fn is_svg_element(&self) -> bool {
        false
    }
    fn style_attribute(&self) -> Option<ArcBorrow<'_, Locked<PropertyDeclarationBlock>>> {
        self.slot().style_attr.as_ref().map(|a| a.borrow_arc())
    }
    fn animation_rule(
        &self,
        _: &SharedStyleContext,
    ) -> Option<Arc<Locked<PropertyDeclarationBlock>>> {
        None
    }
    fn transition_rule(
        &self,
        _: &SharedStyleContext,
    ) -> Option<Arc<Locked<PropertyDeclarationBlock>>> {
        None
    }
    fn state(&self) -> stylo_dom::ElementState {
        stylo_dom::ElementState::empty()
    }
    fn has_part_attr(&self) -> bool {
        false
    }
    fn exports_any_part(&self) -> bool {
        false
    }
    fn id(&self) -> Option<&Atom> {
        self.slot().dom_id.as_ref()
    }
    fn each_class<F>(&self, mut callback: F)
    where
        F: FnMut(&AtomIdent),
    {
        for c in &self.slot().classes {
            callback(c);
        }
    }
    fn each_custom_state<F>(&self, _: F)
    where
        F: FnMut(&AtomIdent),
    {
    }
    fn each_attr_name<F>(&self, mut callback: F)
    where
        F: FnMut(&LocalName),
    {
        for (name, _) in &self.slot().attrs {
            callback(LocalName::cast(name));
        }
    }
    fn has_dirty_descendants(&self) -> bool {
        self.slot().dirty_descendants.load(Ordering::Relaxed)
    }
    fn has_snapshot(&self) -> bool {
        false
    }
    fn handled_snapshot(&self) -> bool {
        self.slot().handled_snapshot.get()
    }
    unsafe fn set_handled_snapshot(&self) {
        self.slot().handled_snapshot.set(true);
    }
    unsafe fn set_dirty_descendants(&self) {
        self.slot().dirty_descendants.store(true, Ordering::Relaxed);
    }
    unsafe fn unset_dirty_descendants(&self) {
        self.slot().dirty_descendants.store(false, Ordering::Relaxed);
    }
    fn store_children_to_process(&self, n: isize) {
        self.slot().children_to_process.store(n, Ordering::Relaxed);
    }
    fn did_process_child(&self) -> isize {
        self.slot()
            .children_to_process
            .fetch_sub(1, Ordering::Relaxed)
            - 1
    }
    unsafe fn ensure_data(&self) -> ElementDataMut<'_> {
        self.slot().data.borrow_mut()
    }
    unsafe fn clear_data(&self) {
        *self.slot().data.borrow_mut() = ElementData::default();
    }
    fn has_data(&self) -> bool {
        true
    }
    fn borrow_data(&self) -> Option<ElementDataRef<'_>> {
        Some(self.slot().data.borrow())
    }
    fn mutate_data(&self) -> Option<ElementDataMut<'_>> {
        Some(self.slot().data.borrow_mut())
    }
    fn skip_item_display_fixup(&self) -> bool {
        false
    }
    fn may_have_animations(&self) -> bool {
        false
    }
    fn has_animations(&self, _: &SharedStyleContext) -> bool {
        false
    }
    fn has_css_animations(&self, _: &SharedStyleContext, _: Option<PseudoElement>) -> bool {
        false
    }
    fn has_css_transitions(&self, _: &SharedStyleContext, _: Option<PseudoElement>) -> bool {
        false
    }
    fn shadow_root(&self) -> Option<NoShadow> {
        None
    }
    fn containing_shadow(&self) -> Option<NoShadow> {
        None
    }
    fn lang_attr(&self) -> Option<AttrValue> {
        None
    }
    fn match_element_lang(&self, _: Option<Option<AttrValue>>, _: &Lang) -> bool {
        false
    }
    fn is_html_document_body_element(&self) -> bool {
        false
    }
    fn synthesize_presentational_hints_for_legacy_attributes<V>(
        &self,
        _: selectors::matching::VisitedHandlingMode,
        _: &mut V,
    ) where
        V: Push<ApplicableDeclarationBlock>,
    {
    }
    fn local_name(&self) -> &WebLocalName {
        &self.slot().tag
    }
    fn namespace(&self) -> &WebNs {
        &self.slot().ns
    }
    fn query_container_size(&self, _: &Display) -> euclid::default::Size2D<Option<app_units::Au>> {
        euclid::default::Size2D::new(None, None)
    }
    fn has_selector_flags(&self, flags: ElementSelectorFlags) -> bool {
        self.slot().selector_flags.get().contains(flags)
    }
    fn relative_selector_search_direction(&self) -> ElementSelectorFlags {
        ElementSelectorFlags::empty()
    }
    fn get_attr(&self, attr: &LocalName, namespace: &Namespace) -> Option<String> {
        if !namespace.0.is_empty() {
            return None;
        }
        let want: &WebLocalName = attr;
        self.slot()
            .attrs
            .iter()
            .find(|(n, _)| n == want)
            .map(|(_, v)| v.clone())
    }
}

// Silence unused imports that vary by stylo version.
#[allow(dead_code)]
fn _types(_: AtomString, _: RestyleDamage, _: *mut c_void) {}
