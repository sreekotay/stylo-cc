mod host;

use std::env;
use std::fs;
use std::time::Instant;

use euclid::{Scale, Size2D};
use host::{Elem, HostDoc};
use style::animation::DocumentAnimationSet;
use style::context::{
    QuirksMode, RegisteredSpeculativePainter, RegisteredSpeculativePainters, SharedStyleContext,
    StyleSystemOptions,
};
use style::device::Device;
use style::driver;
use style::font_metrics::FontMetrics;
use style::global_style_data::STYLE_THREAD_POOL;
use style::media_queries::MediaType;
use style::properties::ComputedValues;
use style::queries::values::PrefersColorScheme;
use style::selector_parser::{PseudoElement, SnapshotMap};
use style::servo::media_features::PointerCapabilities;
use style::shared_lock::StylesheetGuards;
use style::media_queries::MediaList;
use style::stylesheets::{
    AllowImportRules, DocumentStyleSheet, Origin, Stylesheet, UrlExtraData,
};
use style::stylist::Stylist;
use style::traversal::{recalc_style_at, DomTraversal, PerLevelTraversalData};
use style::traversal_flags::TraversalFlags;
use style::values::computed::Length;
use style::values::computed::font::GenericFontFamily;
use style::values::specified::font::QueryFontMetricsFlags;
use style::Atom;
use style::servo_arc::Arc;
use stylebench_fixture::{Fixture, Mut};

#[derive(Debug)]
struct DummyFonts;

impl style::device::servo::FontMetricsProvider for DummyFonts {
    fn query_font_metrics(
        &self,
        _: bool,
        _: &style::properties::style_structs::Font,
        _: style::values::computed::CSSPixelLength,
        _: QueryFontMetricsFlags,
    ) -> FontMetrics {
        FontMetrics::default()
    }
    fn base_size_for_generic(&self, _: GenericFontFamily) -> Length {
        Length::new(16.0)
    }
}

struct NoPainters;
impl RegisteredSpeculativePainters for NoPainters {
    fn get(&self, _: &Atom) -> Option<&dyn RegisteredSpeculativePainter> {
        None
    }
}

struct Recalc<'a> {
    shared: SharedStyleContext<'a>,
}

impl<'a> DomTraversal<Elem> for Recalc<'a> {
    fn process_preorder<F>(
        &self,
        traversal_data: &PerLevelTraversalData,
        context: &mut style::context::StyleContext<Elem>,
        node: host::Node,
        note_child: F,
    ) where
        F: FnMut(host::Node),
    {
        if let Some(el) = node.as_element() {
            let mut data = unsafe { el.ensure_data() };
            recalc_style_at(self, traversal_data, context, el, &mut data, note_child);
        }
    }

    fn process_postorder(&self, _: &mut style::context::StyleContext<Elem>, _: host::Node) {}

    fn needs_postorder_traversal() -> bool {
        false
    }

    fn shared_context(&self) -> &SharedStyleContext<'a> {
        &self.shared
    }
}

fn parse_sheet(css: &str, lock: style::shared_lock::SharedRwLock) -> Stylesheet {
    let url = UrlExtraData(Arc::new(url::Url::parse("about:blank").unwrap()));
    let media = Arc::new(lock.wrap(MediaList::empty()));
    Stylesheet::from_str(
        css,
        url,
        Origin::Author,
        media,
        lock,
        None,
        None,
        QuirksMode::NoQuirks,
        AllowImportRules::No,
    )
}

/// prints every one of them.
fn content_longhands() -> Vec<style::properties::LonghandId> {
    use style::properties::{NonCustomPropertyId, PropertyId};
    let mut out = Vec::new();
    for id in NonCustomPropertyId::iter() {
        let Some(lh) = id.as_longhand() else { continue };
        if lh.is_logical() || !PropertyId::NonCustom(id).enabled_for_all_content() {
            continue;
        }
        let name = lh.name();
        if name.starts_with("-x-") || name.starts_with("-servo-") || name.starts_with("-moz-") {
            continue;
        }
        out.push(lh);
    }
    out
}

/// `name \t inherited \t initial-value`, computed (unresolved) form:
/// `currentcolor` stays a keyword, the style adjuster has not run.
fn dump_longhands() {
    let initial = ComputedValues::initial_values_with_font_override(
        style::properties::style_structs::Font::initial_values(),
    );
    for lh in content_longhands() {
        let mut v = String::new();
        initial
            .computed_or_resolved_value(lh, None, &mut v)
            .expect("serialize");
        println!("{}\t{}\t{}", lh.name(), lh.inherited() as u8, v);
    }
}

fn main() {
    style::thread_state::initialize(style::thread_state::ThreadState::LAYOUT);
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "fixtures/tiny.stylebench".into());
    if path == "--longhands" {
        dump_longhands();
        return;
    }
    let text = fs::read_to_string(&path).expect("read fixture");
    let fix = Fixture::parse(&text).expect("parse fixture");

    let mut doc = HostDoc::from_fixture(&fix);
    let lock = doc.lock().clone();

    let make_device = |width: f32| {
        let defaults = ComputedValues::initial_values_with_font_override(
            style::properties::style_structs::Font::initial_values(),
        );
        Device::new(
            MediaType::screen(),
            QuirksMode::NoQuirks,
            Size2D::new(width, 600.0),
            Size2D::new(width, 600.0),
            Scale::new(1.0),
            Box::new(DummyFonts),
            defaults,
            PrefersColorScheme::Light,
            PointerCapabilities::empty(),
            PointerCapabilities::empty(),
        )
    };
    let mut stylist = Stylist::new(make_device(800.0), QuirksMode::NoQuirks);
    let combined = format!("{}\n{}", fix.base_css, fix.css);
    let sheet = parse_sheet(&combined, lock.clone());
    {
        let guard = lock.read();
        stylist.append_stylesheet(DocumentStyleSheet(Arc::new(sheet)), &guard);
        stylist.flush(&StylesheetGuards::same(&guard));
    }

    let mut snapshots = SnapshotMap::new();
    let painters = NoPainters;
    let guard = lock.read();
    let threads = STYLE_THREAD_POOL.num_threads.unwrap_or(1);
    let pool = STYLE_THREAD_POOL.pool();

    let restyle = |stylist: &Stylist, doc: &HostDoc, snapshots: &SnapshotMap| {
        let shared = SharedStyleContext {
            stylist,
            visited_styles_enabled: false,
            options: StyleSystemOptions::default(),
            guards: StylesheetGuards::same(&guard),
            current_time_for_animations: 0.0,
            traversal_flags: TraversalFlags::empty(),
            snapshot_map: snapshots,
            animations: DocumentAnimationSet::default(),
            registered_speculative_painters: &painters,
        };
        let traversal = Recalc { shared };
        let token = Recalc::pre_traverse(doc.root_elem(), traversal.shared_context());
        if token.should_traverse() {
            driver::traverse_dom(&traversal, token, pool.as_ref());
        }
    };

    let t0 = Instant::now();
    restyle(&stylist, &doc, &snapshots);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    snapshots.clear();
    doc.clear_restyle_bits();

    let mut mut_ms = 0.0;
    let mut pending = false;
    for m in &fix.mutations {
        match m {
            Mut::Restyle => {
                if pending {
                    let t1 = Instant::now();
                    restyle(&stylist, &doc, &snapshots);
                    mut_ms += t1.elapsed().as_secs_f64() * 1000.0;
                    snapshots.clear();
                    doc.clear_restyle_bits();
                    pending = false;
                }
            }
            Mut::Resize { width } => {
                // Servo on a viewport change: swap the device, rebuild the
                // origins whose media-affected rules moved, restyle the
                // document from the root. All of it is the step's cost.
                let t1 = Instant::now();
                let guards = StylesheetGuards::same(&guard);
                let origins = stylist.set_device(make_device(*width as f32), &guards);
                if !origins.is_empty() {
                    stylist.force_stylesheet_origins_dirty(origins);
                    stylist.flush(&guards);
                    doc.restyle_root();
                }
                mut_ms += t1.elapsed().as_secs_f64() * 1000.0;
                pending = true;
            }
            _ => {
                doc.apply_mut(m, &mut snapshots);
                pending = true;
            }
        }
    }

    println!("# runner=stylo fixture={path} threads={threads}");
    println!(
        "# TIME_MS={ms:.3} TIME_MUT_MS={mut_ms:.3} elements={}",
        doc.live_count()
    );
    let longhands = content_longhands();
    let line = |idx: usize, tag: &str, id: &str, style: &ComputedValues| -> String {
        let mut out = format!("{}\t{}\tid={}", idx, tag, id);
        for lh in &longhands {
            out.push('\t');
            out.push_str(lh.name());
            out.push('=');
            let mut v = String::new();
            style
                .computed_or_resolved_value(*lh, None, &mut v)
                .expect("serialize");
            out.push_str(&v);
        }
        out
    };
    doc.each_element(|el| {
        let data = el.borrow_data().expect("styled");
        let Some(style) = data.styles.get_primary() else {
            println!("{} unstyled", el.0.debug_id());
            return;
        };
        let id = el
            .id()
            .map(|a| a.as_ref())
            .unwrap_or("-");
        let idx = el.0.debug_id() - 1;
        println!("{}", line(idx, &el.local_name(), id, style));
        // Eager pseudos Stylo kept (content not none / normal), in tree order.
        for pseudo in [PseudoElement::Before, PseudoElement::After] {
            if let Some(ps) = data.styles.pseudos.get(&pseudo) {
                let tag = if pseudo == PseudoElement::Before { "::before" } else { "::after" };
                println!("{}", line(idx, tag, id, ps));
            }
        }
    });
}

use style::dom::{TElement, TNode};
