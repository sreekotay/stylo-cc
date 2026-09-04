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
use style::selector_parser::SnapshotMap;
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
use style_traits::ToCss;
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

fn bg_rgba(style: &ComputedValues) -> (u8, u8, u8, u8) {
    use style::color::ColorSpace;
    let abs = style.resolve_color(&style.get_background().background_color);
    let srgb = abs.to_color_space(ColorSpace::Srgb);
    let r = (srgb.components.0 * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (srgb.components.1 * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (srgb.components.2 * 255.0).round().clamp(0.0, 255.0) as u8;
    let a = (srgb.alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    (r, g, b, a)
}

fn main() {
    style::thread_state::initialize(style::thread_state::ThreadState::LAYOUT);
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "fixtures/tiny.stylebench".into());
    let text = fs::read_to_string(&path).expect("read fixture");
    let fix = Fixture::parse(&text).expect("parse fixture");

    let mut doc = HostDoc::from_fixture(&fix);
    let lock = doc.lock().clone();

    let defaults = ComputedValues::initial_values_with_font_override(
        style::properties::style_structs::Font::initial_values(),
    );
    let device = Device::new(
        MediaType::screen(),
        QuirksMode::NoQuirks,
        Size2D::new(800.0, 600.0),
        Size2D::new(800.0, 600.0),
        Scale::new(1.0),
        Box::new(DummyFonts),
        defaults,
        PrefersColorScheme::Light,
        PointerCapabilities::empty(),
        PointerCapabilities::empty(),
    );
    let mut stylist = Stylist::new(device, QuirksMode::NoQuirks);
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

    let restyle = |doc: &HostDoc, snapshots: &SnapshotMap| {
        let shared = SharedStyleContext {
            stylist: &stylist,
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
    restyle(&doc, &snapshots);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    snapshots.clear();
    doc.clear_restyle_bits();

    let mut mut_ms = 0.0;
    let mut pending = false;
    for m in &fix.mutations {
        if matches!(m, Mut::Restyle) {
            if pending {
                let t1 = Instant::now();
                restyle(&doc, &snapshots);
                mut_ms += t1.elapsed().as_secs_f64() * 1000.0;
                snapshots.clear();
                doc.clear_restyle_bits();
                pending = false;
            }
        } else {
            doc.apply_mut(m, &mut snapshots);
            pending = true;
        }
    }

    println!("# runner=stylo fixture={path} threads={threads}");
    println!(
        "# TIME_MS={ms:.3} TIME_MUT_MS={mut_ms:.3} elements={}",
        doc.live_count()
    );
    doc.each_element(|el| {
        let data = el.borrow_data().expect("styled");
        let Some(style) = data.styles.get_primary() else {
            println!("{} unstyled", el.0.debug_id());
            return;
        };
        let (r, g, b, a) = bg_rgba(style);
        let id = el
            .id()
            .map(|a| a.as_ref())
            .unwrap_or("-");
        println!(
            "{}\t{}\tid={}\tdisp={}\tpos={}\tw={}\th={}\tminw={}\tfs={}\tlh={}\tfw={}\tvis={}\tcolor={}\tbg={r},{g},{b},{a}",
            el.0.debug_id() - 1,
            el.local_name(),
            id,
            style.clone_display().to_css_string(),
            style.clone_position().to_css_string(),
            style.clone_width().to_css_string(),
            style.clone_height().to_css_string(),
            style.clone_min_width().to_css_string(),
            style.clone_font_size().to_css_string(),
            style.clone_line_height().to_css_string(),
            style.clone_font_weight().to_css_string(),
            style.clone_visibility().to_css_string(),
            style.clone_color().to_css_string(),
        );
    });
}

use style::dom::{TElement, TNode};
