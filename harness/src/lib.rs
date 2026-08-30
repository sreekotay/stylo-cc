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
    pub maximum_selector_length: i32,
    pub rule_count: i32,
    pub element_count: i32,
    pub maximum_tree_depth: i32,
    pub maximum_tree_width: i32,
    pub repeating_sequence_chance: f64,
    pub repeating_sequence_maximum_length: i32,
    pub style_seed: i32,
    pub dom_seed: i32,
}

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
            maximum_selector_length: 6,
            rule_count: 5000,
            element_count: 20000,
            maximum_tree_depth: 6,
            maximum_tree_width: 50,
            repeating_sequence_chance: 0.2,
            repeating_sequence_maximum_length: 3,
            style_seed: 1,
            dom_seed: 2,
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
pub struct Fixture {
    pub config: Config,
    pub base_css: String,
    pub css: String,
    pub nodes: Vec<Node>,
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
    let nodes = {
        let mut rng = Random::new(config.dom_seed);
        make_tree(&config, &mut rng)
    };
    Fixture {
        config,
        base_css: BASE_CSS.to_string(),
        css,
        nodes,
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

fn make_compound_selector(cfg: &Config, rng: &mut Random, index: i32, length: i32) -> String {
    let is_first = index == 0;
    let use_id = is_first && rng.chance(cfg.id_chance);
    let use_element = !use_id && rng.chance(cfg.element_chance);
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

impl Fixture {
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
            let mut classes = String::new();
            for (k, c) in n.classes.iter().enumerate() {
                if k > 0 {
                    classes.push(',');
                }
                classes.push_str(c);
            }
            let mut attrs = String::new();
            for (k, a) in n.attrs.iter().enumerate() {
                if k > 0 {
                    attrs.push(',');
                }
                let _ = write!(attrs, "{}={}", a.name, a.value);
            }
            writeln!(
                out,
                "{}\t{}\t{}\t{}\t{}\t{}",
                i,
                n.parent,
                n.tag,
                n.dom_id.as_deref().unwrap_or("-"),
                classes,
                attrs
            )?;
        }
        Ok(())
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut section = "";
        let mut base = String::new();
        let mut css = String::new();
        let mut nodes = Vec::new();
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
        })
    }
}
