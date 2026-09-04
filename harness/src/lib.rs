//! StyleBench generator port and the text fixture both runners eat.
//!
//! LCG and knobs match WebKit
//! `PerformanceTests/StyleBench/resources/style-bench.js`.

use std::fmt::Write as _;
use std::io::{self, Write};

/// Park–Miller LCG (`seed * 16807 % 2147483647`), same as StyleBench `Random`.
#[derive(Clone, Debug)]
pub struct Random {
    seed: i32,
}

impl Random {
    pub fn new(seed: i32) -> Self {
        let mut seed = seed % 2147483647;
        if seed <= 0 {
            seed += 2147483646;
        }
        Self { seed }
    }

    pub fn next(&mut self) -> i32 {
        self.seed = ((self.seed as i64 * 16807) % 2147483647) as i32;
        self.seed
    }

    pub fn under_one(&mut self) -> f64 {
        (self.next() % 1_048_576) as f64 / 1_048_576.0
    }

    pub fn chance(&mut self, chance: f64) -> bool {
        if chance == 0.0 {
            return false;
        }
        self.under_one() < chance
    }

    pub fn number(&mut self, under: i32) -> i32 {
        if under <= 0 {
            return 0;
        }
        self.next() % under
    }

    pub fn number_square_weighted_to_low(&mut self, under: i32) -> i32 {
        if under <= 0 {
            return 0;
        }
        let random = self.under_one();
        ((random * random) * under as f64).floor() as i32
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub name: String,
    pub element_type_count: i32,
    pub id_chance: f64,
    pub element_chance: f64,
    pub class_count: i32,
    pub class_chance: f64,
    pub star_chance: f64,
    pub attribute_chance: f64,
    pub attribute_count: i32,
    pub attribute_value_count: i32,
    pub attribute_operators: Vec<String>,
    pub element_class_chance: f64,
    pub element_maximum_classes: i32,
    pub element_attribute_chance: f64,
    pub element_maximum_attributes: i32,
    pub combinators: Vec<String>,
    pub pseudo_classes: Vec<String>,
    pub pseudo_class_chance: f64,
    pub maximum_selector_length: i32,
    pub rule_count: i32,
    pub element_count: i32,
    pub maximum_tree_depth: i32,
    pub maximum_tree_width: i32,
    pub repeating_sequence_chance: f64,
    pub repeating_sequence_maximum_length: i32,
    pub style_seed: i32,
    pub dom_seed: i32,
    pub leaf_mutation_chance: f64,
    pub step_count: i32,
    pub mutations_per_step: i32,
}

const STRUCTURAL_PSEUDOS: [&str; 6] = [
    "first-child",
    "last-child",
    "first-of-type",
    "last-of-type",
    "only-of-type",
    "empty",
];

impl Config {
    /// StyleBench `defaultConfiguration` (20k / 5k).
    pub fn default_suite() -> Self {
        Self {
            name: "Default".into(),
            element_type_count: 10,
            id_chance: 0.05,
            element_chance: 0.5,
            class_count: 200,
            class_chance: 0.3,
            star_chance: 0.05,
            attribute_chance: 0.02,
            attribute_count: 10,
            attribute_value_count: 20,
            attribute_operators: vec!["".into(), "=".into()],
            element_class_chance: 0.5,
            element_maximum_classes: 3,
            element_attribute_chance: 0.2,
            element_maximum_attributes: 3,
            combinators: vec![" ".into(), ">".into()],
            pseudo_classes: vec![],
            pseudo_class_chance: 0.0,
            maximum_selector_length: 6,
            rule_count: 5000,
            element_count: 20000,
            maximum_tree_depth: 6,
            maximum_tree_width: 50,
            repeating_sequence_chance: 0.2,
            repeating_sequence_maximum_length: 3,
            style_seed: 1,
            dom_seed: 2,
            leaf_mutation_chance: 0.1,
            step_count: 5,
            mutations_per_step: 100,
        }
    }

    /// Same knobs, small enough for a compile / cmp loop.
    pub fn tiny() -> Self {
        let mut c = Self::default_suite();
        c.name = "Tiny".into();
        c.element_type_count = 6;
        c.class_count = 20;
        c.attribute_count = 6;
        c.attribute_value_count = 8;
        c.rule_count = 40;
        c.element_count = 80;
        c.maximum_tree_depth = 4;
        c.maximum_tree_width = 8;
        c.step_count = 1;
        c.mutations_per_step = 8;
        c
    }

    /// StyleBench `siblingCombinatorConfiguration`. Same seeds as default.
    pub fn sibling_suite() -> Self {
        let mut c = Self::default_suite();
        c.name = "Sibling".into();
        c.combinators = vec![
            " ".into(),
            " ".into(),
            ">".into(),
            ">".into(),
            "~".into(),
            "+".into(),
        ];
        c
    }

    /// StyleBench `structuralPseudoClassConfiguration`. Same seeds as default.
    pub fn structural_suite() -> Self {
        let mut c = Self::default_suite();
        c.name = "Structural".into();
        c.pseudo_class_chance = 0.1;
        c.pseudo_classes = STRUCTURAL_PSEUDOS.iter().map(|s| s.to_string()).collect();
        c
    }

    /// Tiny tree / sheet with the structural pseudo-class mix.
    pub fn tiny_structural() -> Self {
        let mut c = Self::tiny();
        c.name = "TinyStructural".into();
        c.pseudo_class_chance = 0.1;
        c.pseudo_classes = STRUCTURAL_PSEUDOS.iter().map(|s| s.to_string()).collect();
        c
    }

    /// Tiny tree / sheet with the sibling combinator mix.
    pub fn tiny_sibling() -> Self {
        let mut c = Self::tiny();
        c.name = "TinySibling".into();
        c.combinators = vec![
            " ".into(),
            " ".into(),
            ">".into(),
            ">".into(),
            "~".into(),
            "+".into(),
        ];
        c
    }
}

#[derive(Clone, Debug)]
pub struct Attr {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub parent: i32,
    pub tag: String,
    pub dom_id: Option<String>,
    pub classes: Vec<String>,
    pub attrs: Vec<Attr>,
}

#[derive(Clone, Debug)]
pub enum Mut {
    AddClass { id: i32, class: String },
    RemoveClass { id: i32 },
    SetAttr { id: i32, name: String, value: String },
    RemoveAttr { id: i32, name: String },
    AddLeaf {
        id: i32,
        parent: i32,
        at: i32,
        tag: String,
        dom_id: Option<String>,
        classes: Vec<String>,
        attrs: Vec<Attr>,
    },
    RemoveLeaf { id: i32 },
    Restyle,
}

#[derive(Clone, Debug)]
pub struct Fixture {
    pub config: Config,
    pub base_css: String,
    pub css: String,
    pub nodes: Vec<Node>,
    pub mutations: Vec<Mut>,
}

pub const BASE_CSS: &str = "\
#testroot {
  font-size: 10px;
  line-height: 10px;
}
#testroot * {
  display: inline-block;
  height:10px;
  min-width:10px;
}
";

pub fn generate(config: Config) -> Fixture {
    let css = {
        let mut rng = Random::new(config.style_seed);
        make_stylesheet(&config, &mut rng, config.rule_count)
    };
    let mut rng = Random::new(config.dom_seed);
    let nodes = make_tree(&config, &mut rng);
    let mutations = generate_mutations(&config, &mut rng, &nodes);
    Fixture {
        config,
        base_css: BASE_CSS.to_string(),
        css,
        nodes,
        mutations,
    }
}

fn make_stylesheet(cfg: &Config, rng: &mut Random, size: i32) -> String {
    let mut css = String::new();
    for _ in 0..size {
        css.push_str(&make_rule(cfg, rng));
        css.push('\n');
    }
    css
}

fn make_rule(cfg: &Config, rng: &mut Random) -> String {
    let selector = make_selector(cfg, rng);
    let declaration = make_declaration(rng);
    format!("{selector} {{ {declaration} }}")
}

fn make_declaration(rng: &mut Random) -> String {
    format!(
        "background-color: rgb({}, {}, {});",
        rng.next() % 256,
        rng.next() % 256,
        rng.next() % 256
    )
}

fn make_selector(cfg: &Config, rng: &mut Random) -> String {
    let length = rng.number(cfg.maximum_selector_length) + 1;
    let mut result = make_compound_selector(cfg, rng, 0, length);
    for i in 1..length {
        let combinator = &cfg.combinators[rng.number(cfg.combinators.len() as i32) as usize];
        if combinator != " " {
            result.push(' ');
            result.push_str(combinator);
        }
        result.push(' ');
        result.push_str(&make_compound_selector(cfg, rng, i, length));
    }
    result
}

/// StyleBench `randomPseudoClass`: `:empty` only on the subject, redraw otherwise.
fn random_pseudo_class(cfg: &Config, rng: &mut Random, is_last: bool) -> String {
    let pc = &cfg.pseudo_classes[rng.number(cfg.pseudo_classes.len() as i32) as usize];
    if !is_last && pc == "empty" {
        return random_pseudo_class(cfg, rng, is_last);
    }
    pc.clone()
}

fn make_compound_selector(cfg: &Config, rng: &mut Random, index: i32, length: i32) -> String {
    let is_first = index == 0;
    let is_last = index == length - 1;
    // JS `chance(p) && list.length`: the draw happens first; chance(0) draws nothing.
    let use_pseudo_class = rng.chance(cfg.pseudo_class_chance) && !cfg.pseudo_classes.is_empty();
    let use_id = is_first && rng.chance(cfg.id_chance);
    // :nth-of-type etc only make sense with an element.
    let use_element = !use_id && (use_pseudo_class || rng.chance(cfg.element_chance));
    let use_attribute = !use_id && rng.chance(cfg.attribute_chance);
    let use_id_element_or_attribute = use_id || use_element || use_attribute;
    let use_star = !use_id_element_or_attribute && !is_first && rng.chance(cfg.star_chance);
    let use_class =
        !use_id && !use_star && (!use_id_element_or_attribute || rng.chance(cfg.class_chance));
    let mut result = String::new();
    if use_element {
        result.push_str(&random_element_name(cfg, rng));
    }
    if use_star {
        result = "*".into();
    }
    if use_id {
        result.push('#');
        result.push_str(&random_id(cfg, rng));
    }
    if use_class {
        let class_count = rng.number_square_weighted_to_low(2) + 1;
        for _ in 0..class_count {
            result.push('.');
            result.push_str(&random_class_name_from_range(
                cfg,
                rng,
                (index + 1) as f64 / length as f64,
            ));
        }
    }
    if use_attribute {
        result.push_str(&random_attribute_selector(cfg, rng));
    }
    if use_pseudo_class {
        result.push(':');
        result.push_str(&random_pseudo_class(cfg, rng, is_last));
    }
    result
}

fn random_element_name(cfg: &Config, rng: &mut Random) -> String {
    format!(
        "elem{}",
        rng.number_square_weighted_to_low(cfg.element_type_count)
    )
}

fn random_class_name(cfg: &Config, rng: &mut Random) -> String {
    format!(
        "class{}",
        rng.number_square_weighted_to_low(cfg.class_count)
    )
}

fn random_class_name_from_range(cfg: &Config, rng: &mut Random, range: f64) -> String {
    let maximum = (range * cfg.class_count as f64).round() as i32;
    format!("class{}", rng.number_square_weighted_to_low(maximum))
}

fn random_id(cfg: &Config, rng: &mut Random) -> String {
    let id_count = (cfg.id_chance * cfg.element_count as f64) as i32;
    format!("id{}", rng.number(id_count.max(1)))
}

fn random_attribute_name(cfg: &Config, rng: &mut Random) -> String {
    format!(
        "attr{}",
        rng.number_square_weighted_to_low(cfg.attribute_count)
    )
}

fn random_attribute_value(cfg: &Config, rng: &mut Random) -> String {
    let value_num = rng.number_square_weighted_to_low(cfg.attribute_value_count);
    if value_num == 0 {
        return String::new();
    }
    if value_num == 1 {
        return "val".into();
    }
    format!("val{value_num}")
}

fn random_attribute_selector(cfg: &Config, rng: &mut Random) -> String {
    let name = random_attribute_name(cfg, rng);
    let op = &cfg.attribute_operators
        [rng.number_square_weighted_to_low(cfg.attribute_operators.len() as i32) as usize];
    if op.is_empty() {
        return format!("[{name}]");
    }
    let value = random_attribute_value(cfg, rng);
    format!("[{name}{op}\"{value}\"]")
}

fn make_element(cfg: &Config, rng: &mut Random, id_count: &mut i32) -> Node {
    let mut node = Node {
        parent: -1,
        tag: random_element_name(cfg, rng),
        dom_id: None,
        classes: Vec::new(),
        attrs: Vec::new(),
    };
    if rng.chance(cfg.element_class_chance) {
        let count = rng.number_square_weighted_to_low(cfg.element_maximum_classes) + 1;
        for _ in 0..count {
            node.classes.push(random_class_name(cfg, rng));
        }
    }
    if rng.chance(cfg.element_attribute_chance) {
        let count = rng.number(cfg.element_maximum_attributes) + 1;
        for _ in 0..count {
            node.attrs.push(Attr {
                name: random_attribute_name(cfg, rng),
                value: random_attribute_value(cfg, rng),
            });
        }
    }
    if rng.chance(cfg.id_chance) {
        node.dom_id = Some(format!("id{id_count}"));
        *id_count += 1;
    }
    node
}

fn subtree_size(nodes: &[Node], root: usize) -> i32 {
    let mut n = 1i32;
    for (i, node) in nodes.iter().enumerate() {
        if node.parent == root as i32 {
            n += subtree_size(nodes, i);
        }
    }
    n
}

fn clone_subtree(nodes: &mut Vec<Node>, src: usize, new_parent: i32) -> i32 {
    let new_idx = nodes.len();
    let mut copy = nodes[src].clone();
    copy.parent = new_parent;
    nodes.push(copy);
    let children: Vec<usize> = (0..new_idx)
        .filter(|&i| nodes[i].parent == src as i32)
        .collect();
    let mut added = 1;
    for c in children {
        added += clone_subtree(nodes, c, new_idx as i32);
    }
    added
}

fn make_tree(cfg: &Config, rng: &mut Random) -> Vec<Node> {
    let mut nodes = vec![Node {
        parent: -1,
        tag: "div".into(),
        dom_id: Some("testroot".into()),
        classes: Vec::new(),
        attrs: Vec::new(),
    }];
    let mut id_count = 0i32;
    let mut remaining = cfg.element_count;
    make_tree_with_depth(
        cfg,
        rng,
        &mut nodes,
        0,
        &mut remaining,
        0,
        &mut id_count,
    );
    nodes
}

fn make_tree_with_depth(
    cfg: &Config,
    rng: &mut Random,
    nodes: &mut Vec<Node>,
    parent: usize,
    remaining: &mut i32,
    depth: i32,
    id_count: &mut i32,
) {
    if *remaining <= 0 {
        return;
    }
    let maximum_depth = cfg.maximum_tree_depth;
    let maximum_width = cfg.maximum_tree_width;
    let non_empty_chance = (maximum_depth - depth) as f64 / maximum_depth as f64;
    let should_repeat = rng.chance(cfg.repeating_sequence_chance);
    let repeating_sequence_length = if should_repeat {
        rng.number(cfg.repeating_sequence_maximum_length) + 1
    } else {
        0
    };

    let child_count = if depth == 0 {
        *remaining
    } else if rng.chance(non_empty_chance) {
        rng.number(maximum_width * depth / maximum_depth)
    } else {
        0
    };

    let mut repeating_sequence: Vec<usize> = Vec::new();
    let mut repeating_sequence_size = 0i32;
    for _ in 0..child_count {
        if *remaining <= 0 {
            return;
        }
        if should_repeat
            && repeating_sequence.len() == repeating_sequence_length as usize
            && repeating_sequence_size < *remaining
        {
            for &src in &repeating_sequence {
                let added = clone_subtree(nodes, src, parent as i32);
                *remaining -= added;
                if *remaining <= 0 {
                    return;
                }
            }
            continue;
        }
        let mut el = make_element(cfg, rng, id_count);
        el.parent = parent as i32;
        nodes.push(el);
        *remaining -= 1;
        if *remaining <= 0 {
            return;
        }
        let child = nodes.len() - 1;
        make_tree_with_depth(cfg, rng, nodes, child, remaining, depth + 1, id_count);
        if *remaining <= 0 {
            return;
        }
        if should_repeat && (repeating_sequence.len() as i32) < repeating_sequence_length {
            repeating_sequence.push(child);
            repeating_sequence_size += subtree_size(nodes, child);
        }
    }
}

fn next_dom_id_count(nodes: &[Node]) -> i32 {
    let mut max = -1i32;
    for n in nodes {
        if let Some(id) = &n.dom_id {
            if let Some(rest) = id.strip_prefix("id") {
                if let Ok(v) = rest.parse::<i32>() {
                    max = max.max(v);
                }
            }
        }
    }
    max + 1
}

fn live_child(children: &[Vec<usize>], dead: &[bool], i: usize) -> bool {
    children[i].iter().any(|&c| !dead[c])
}

fn tree_order(children: &[Vec<usize>], dead: &[bool]) -> Vec<usize> {
    let mut out = Vec::new();
    fn walk(i: usize, children: &[Vec<usize>], dead: &[bool], out: &mut Vec<usize>) {
        if i != 0 && !dead[i] {
            out.push(i);
        }
        for &c in &children[i] {
            if !dead[c] {
                walk(c, children, dead, out);
            }
        }
    }
    walk(0, children, dead, &mut out);
    out
}

/// StyleBench `makeSteps`: same LCG as the tree, same skip rules.
fn generate_mutations(cfg: &Config, rng: &mut Random, initial: &[Node]) -> Vec<Mut> {
    let mut nodes = initial.to_vec();
    let mut dead = vec![false; nodes.len()];
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (i, n) in nodes.iter().enumerate() {
        if n.parent >= 0 {
            children[n.parent as usize].push(i);
        }
    }
    let mut id_count = next_dom_id_count(&nodes);
    let mut live = tree_order(&children, &dead);
    let mut out = Vec::new();

    let pick = |rng: &mut Random, live: &[usize]| -> Option<usize> {
        if live.is_empty() {
            return None;
        }
        Some(live[rng.number(live.len() as i32) as usize])
    };

    for _ in 0..cfg.step_count {
        let mut n = 0;
        while n < cfg.mutations_per_step {
            let Some(i) = pick(rng, &live) else { break };
            if !live_child(&children, &dead, i) && !rng.chance(cfg.leaf_mutation_chance) {
                continue;
            }
            let class = random_class_name(cfg, rng);
            if !nodes[i].classes.iter().any(|c| c == &class) {
                nodes[i].classes.push(class.clone());
            }
            out.push(Mut::AddClass {
                id: i as i32,
                class,
            });
            n += 1;
        }
        out.push(Mut::Restyle);

        n = 0;
        while n < cfg.mutations_per_step {
            let Some(i) = pick(rng, &live) else { break };
            if !live_child(&children, &dead, i) && !rng.chance(cfg.leaf_mutation_chance) {
                continue;
            }
            if nodes[i].classes.is_empty() {
                continue;
            }
            nodes[i].classes.remove(0);
            out.push(Mut::RemoveClass { id: i as i32 });
            n += 1;
        }
        out.push(Mut::Restyle);

        n = 0;
        while n < cfg.mutations_per_step {
            let Some(i) = pick(rng, &live) else { break };
            if !live_child(&children, &dead, i) && !rng.chance(cfg.leaf_mutation_chance) {
                continue;
            }
            let mut names: Vec<String> = Vec::new();
            if !nodes[i].classes.is_empty() {
                names.push("class".into());
            }
            for a in &nodes[i].attrs {
                names.push(a.name.clone());
            }
            if nodes[i].dom_id.is_some() {
                names.push("id".into());
            }
            let mut mutated = false;
            for name in names {
                if name == "class" || name == "id" {
                    continue;
                }
                if rng.chance(0.5) {
                    nodes[i].attrs.retain(|a| a.name != name);
                    out.push(Mut::RemoveAttr {
                        id: i as i32,
                        name,
                    });
                } else {
                    let value = random_attribute_value(cfg, rng);
                    if let Some(a) = nodes[i].attrs.iter_mut().find(|a| a.name == name) {
                        a.value = value.clone();
                    }
                    out.push(Mut::SetAttr {
                        id: i as i32,
                        name,
                        value,
                    });
                }
                mutated = true;
            }
            if !mutated {
                let count = rng.number(cfg.element_maximum_attributes) + 1;
                for _ in 0..count {
                    let name = random_attribute_name(cfg, rng);
                    let value = random_attribute_value(cfg, rng);
                    if let Some(a) = nodes[i].attrs.iter_mut().find(|a| a.name == name) {
                        a.value = value.clone();
                    } else {
                        nodes[i].attrs.push(Attr {
                            name: name.clone(),
                            value: value.clone(),
                        });
                    }
                    out.push(Mut::SetAttr {
                        id: i as i32,
                        name,
                        value,
                    });
                }
            }
            n += 1;
        }
        out.push(Mut::Restyle);

        n = 0;
        while n < cfg.mutations_per_step {
            let Some(p) = pick(rng, &live) else { break };
            if !live_child(&children, &dead, p) {
                continue;
            }
            let at = rng.number(children[p].iter().filter(|&&c| !dead[c]).count() as i32 + 1);
            let mut el = make_element(cfg, rng, &mut id_count);
            el.parent = p as i32;
            let id = nodes.len() as i32;
            nodes.push(el.clone());
            dead.push(false);
            children.push(Vec::new());
            let live_kids: Vec<usize> = children[p].iter().copied().filter(|&c| !dead[c]).collect();
            let insert_at = at as usize;
            if insert_at >= live_kids.len() {
                children[p].push(id as usize);
            } else {
                let before = live_kids[insert_at];
                let pos = children[p].iter().position(|&c| c == before).unwrap();
                children[p].insert(pos, id as usize);
            }
            out.push(Mut::AddLeaf {
                id,
                parent: p as i32,
                at,
                tag: el.tag,
                dom_id: el.dom_id,
                classes: el.classes,
                attrs: el.attrs,
            });
            n += 1;
        }
        live = tree_order(&children, &dead);
        out.push(Mut::Restyle);

        n = 0;
        while n < cfg.mutations_per_step {
            let Some(i) = pick(rng, &live) else { break };
            if live_child(&children, &dead, i) {
                continue;
            }
            if nodes[i].parent < 0 {
                continue;
            }
            let p = nodes[i].parent as usize;
            children[p].retain(|&c| c != i);
            dead[i] = true;
            out.push(Mut::RemoveLeaf { id: i as i32 });
            n += 1;
        }
        live = tree_order(&children, &dead);
        out.push(Mut::Restyle);
    }
    out
}

fn fmt_classes(classes: &[String]) -> String {
    classes.join(",")
}

fn fmt_attrs(attrs: &[Attr]) -> String {
    let mut s = String::new();
    for (k, a) in attrs.iter().enumerate() {
        if k > 0 {
            s.push(',');
        }
        let _ = write!(s, "{}={}", a.name, a.value);
    }
    s
}

impl Fixture {
    pub fn leaf_adds(&self) -> usize {
        self.mutations
            .iter()
            .filter(|m| matches!(m, Mut::AddLeaf { .. }))
            .count()
    }

    pub fn write(&self, mut out: impl Write) -> io::Result<()> {
        writeln!(out, "# stylebench-fixture 1")?;
        writeln!(
            out,
            "# name={} elements={} rules={}",
            self.config.name,
            self.nodes.len(),
            self.config.rule_count
        )?;
        writeln!(out, "---base---")?;
        write!(out, "{}", self.base_css)?;
        if !self.base_css.ends_with('\n') {
            writeln!(out)?;
        }
        writeln!(out, "---css---")?;
        write!(out, "{}", self.css)?;
        if !self.css.ends_with('\n') {
            writeln!(out)?;
        }
        writeln!(out, "---tree---")?;
        for (i, n) in self.nodes.iter().enumerate() {
            writeln!(
                out,
                "{}\t{}\t{}\t{}\t{}\t{}",
                i,
                n.parent,
                n.tag,
                n.dom_id.as_deref().unwrap_or("-"),
                fmt_classes(&n.classes),
                fmt_attrs(&n.attrs)
            )?;
        }
        writeln!(out, "---mut---")?;
        for m in &self.mutations {
            match m {
                Mut::AddClass { id, class } => writeln!(out, "+class\t{id}\t{class}")?,
                Mut::RemoveClass { id } => writeln!(out, "-class\t{id}")?,
                Mut::SetAttr { id, name, value } => {
                    writeln!(out, "+attr\t{id}\t{name}={value}")?
                }
                Mut::RemoveAttr { id, name } => writeln!(out, "-attr\t{id}\t{name}")?,
                Mut::AddLeaf {
                    id,
                    parent,
                    at,
                    tag,
                    dom_id,
                    classes,
                    attrs,
                } => writeln!(
                    out,
                    "+leaf\t{id}\t{parent}\t{at}\t{tag}\t{}\t{}\t{}",
                    dom_id.as_deref().unwrap_or("-"),
                    fmt_classes(classes),
                    fmt_attrs(attrs)
                )?,
                Mut::RemoveLeaf { id } => writeln!(out, "-leaf\t{id}")?,
                Mut::Restyle => writeln!(out, "restyle")?,
            }
        }
        Ok(())
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut section = "";
        let mut base = String::new();
        let mut css = String::new();
        let mut nodes = Vec::new();
        let mut mutations = Vec::new();
        let mut name = "parsed".to_string();
        for line in text.lines() {
            // Fixture comments are `# …` (hash + space). `#testroot` / `#id0` are CSS.
            if let Some(rest) = line.strip_prefix("# ") {
                if let Some(n) = rest.strip_prefix("name=") {
                    name = n.split_whitespace().next().unwrap_or("parsed").into();
                }
                continue;
            }
            match line {
                "---base---" => {
                    section = "base";
                    continue;
                }
                "---css---" => {
                    section = "css";
                    continue;
                }
                "---tree---" => {
                    section = "tree";
                    continue;
                }
                "---mut---" => {
                    section = "mut";
                    continue;
                }
                _ => {}
            }
            match section {
                "base" => {
                    base.push_str(line);
                    base.push('\n');
                }
                "css" => {
                    css.push_str(line);
                    css.push('\n');
                }
                "tree" => {
                    let cols: Vec<&str> = line.split('\t').collect();
                    if cols.len() < 4 {
                        return Err(format!("bad tree line: {line}"));
                    }
                    let parent: i32 = cols[1].parse().map_err(|_| "parent")?;
                    let dom_id = if cols[3] == "-" {
                        None
                    } else {
                        Some(cols[3].to_string())
                    };
                    let classes = if cols.len() > 4 && !cols[4].is_empty() {
                        cols[4].split(',').map(|s| s.to_string()).collect()
                    } else {
                        Vec::new()
                    };
                    let mut attrs = Vec::new();
                    if cols.len() > 5 && !cols[5].is_empty() {
                        for pair in cols[5].split(',') {
                            let (n, v) = pair.split_once('=').unwrap_or((pair, ""));
                            attrs.push(Attr {
                                name: n.to_string(),
                                value: v.to_string(),
                            });
                        }
                    }
                    nodes.push(Node {
                        parent,
                        tag: cols[2].to_string(),
                        dom_id,
                        classes,
                        attrs,
                    });
                }
                "mut" => {
                    if line.is_empty() {
                        continue;
                    }
                    mutations.push(parse_mut_line(line)?);
                }
                _ => {}
            }
        }
        if nodes.is_empty() {
            return Err("no tree".into());
        }
        let mut config = Config::tiny();
        config.name = name;
        config.element_count = nodes.len() as i32;
        Ok(Fixture {
            config,
            base_css: base,
            css,
            nodes,
            mutations,
        })
    }
}

fn parse_classes(cell: &str) -> Vec<String> {
    if cell.is_empty() {
        Vec::new()
    } else {
        cell.split(',').map(|s| s.to_string()).collect()
    }
}

fn parse_attrs(cell: &str) -> Vec<Attr> {
    let mut attrs = Vec::new();
    if cell.is_empty() {
        return attrs;
    }
    for pair in cell.split(',') {
        let (n, v) = pair.split_once('=').unwrap_or((pair, ""));
        attrs.push(Attr {
            name: n.to_string(),
            value: v.to_string(),
        });
    }
    attrs
}

fn parse_mut_line(line: &str) -> Result<Mut, String> {
    if line == "restyle" {
        return Ok(Mut::Restyle);
    }
    let cols: Vec<&str> = line.split('\t').collect();
    match cols[0] {
        "+class" if cols.len() >= 3 => Ok(Mut::AddClass {
            id: cols[1].parse().map_err(|_| "id")?,
            class: cols[2].to_string(),
        }),
        "-class" if cols.len() >= 2 => Ok(Mut::RemoveClass {
            id: cols[1].parse().map_err(|_| "id")?,
        }),
        "+attr" if cols.len() >= 3 => {
            let (name, value) = cols[2].split_once('=').unwrap_or((cols[2], ""));
            Ok(Mut::SetAttr {
                id: cols[1].parse().map_err(|_| "id")?,
                name: name.to_string(),
                value: value.to_string(),
            })
        }
        "-attr" if cols.len() >= 3 => Ok(Mut::RemoveAttr {
            id: cols[1].parse().map_err(|_| "id")?,
            name: cols[2].to_string(),
        }),
        "+leaf" if cols.len() >= 6 => Ok(Mut::AddLeaf {
            id: cols[1].parse().map_err(|_| "id")?,
            parent: cols[2].parse().map_err(|_| "parent")?,
            at: cols[3].parse().map_err(|_| "at")?,
            tag: cols[4].to_string(),
            dom_id: if cols[5] == "-" {
                None
            } else {
                Some(cols[5].to_string())
            },
            classes: if cols.len() > 6 {
                parse_classes(cols[6])
            } else {
                Vec::new()
            },
            attrs: if cols.len() > 7 {
                parse_attrs(cols[7])
            } else {
                Vec::new()
            },
        }),
        "-leaf" if cols.len() >= 2 => Ok(Mut::RemoveLeaf {
            id: cols[1].parse().map_err(|_| "id")?,
        }),
        _ => Err(format!("bad mut line: {line}")),
    }
}
