//! Grid-based graph documentation. Connectors join only through matching ports;
//! identifiers terminate paths, so a chain never implies transitive edges.
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashSet;

use super::graph::Graph;

const DIRECTIONS: [(i32, i32); 8] = [
    (-1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
];

fn ports(c: char) -> &'static [usize] {
    match c {
        '-' | '─' | '━' | '>' | '<' | '→' | '←' => &[2, 6],
        '|' | '│' | '┃' | '↑' | '↓' => &[0, 4],
        '/' | '╱' => &[1, 5],
        '\\' | '╲' => &[3, 7],
        '+' | '┼' | '╋' => &[0, 2, 4, 6],
        '┌' | '╭' => &[2, 4],
        '┐' | '╮' => &[4, 6],
        '└' | '╰' => &[0, 2],
        '┘' | '╯' => &[0, 6],
        '├' => &[0, 2, 4],
        '┤' => &[0, 4, 6],
        '┬' => &[2, 4, 6],
        '┴' => &[0, 2, 6],
        _ => &[],
    }
}

fn arrow(c: char) -> Option<usize> {
    match c {
        '>' | '→' => Some(2),
        '<' | '←' => Some(6),
        '↑' => Some(0),
        '↓' => Some(4),
        _ => None,
    }
}

#[derive(Default)]
struct Diagram {
    nodes: BTreeSet<String>,
    edges: BTreeSet<(String, String, bool)>,
}

impl Diagram {
    fn parse(text: &str) -> Result<Self, String> {
        let mut result = Self::default();
        let mut cells = BTreeMap::new();
        let mut labels = BTreeMap::new();
        for (row, line) in text.lines().enumerate() {
            let chars: Vec<_> = line.chars().collect();
            let mut col = 0;
            while col < chars.len() {
                // syn supplies Rust's Unicode identifier rules, including raw identifiers.
                let start = col;
                let raw = chars[col..].starts_with(&['r', '#']);
                let first = col + if raw { 2 } else { 0 };
                let mut end = first;
                while end < chars.len() {
                    let candidate: String = chars[first..=end].iter().collect();
                    if !syn::parse_str::<syn::Ident>(&format!("{candidate}__"))
                        .is_ok_and(|ident| ident == format!("{candidate}__"))
                    {
                        break;
                    }
                    end += 1;
                }
                if end > first {
                    let name: String = chars[first..end].iter().collect();
                    result.nodes.insert(name.clone());
                    for c in start..end {
                        labels.insert((row as i32, c as i32), (name.clone(), (row, start)));
                    }
                    col = end;
                } else {
                    let c = chars[col];
                    if !c.is_whitespace() {
                        if ports(c).is_empty() {
                            return Err(format!("Unsupported graph character `{c}`"));
                        }
                        cells.insert((row as i32, col as i32), c);
                    }
                    col += 1;
                }
            }
        }
        let max_col = cells
            .keys()
            .chain(labels.keys())
            .map(|p| p.1)
            .max()
            .unwrap_or(0);
        for (&position, (source, occurrence)) in &labels {
            let mut pending = vec![(position, false)];
            let mut visited = HashSet::new();
            while let Some((position, directed)) = pending.pop() {
                if !visited.insert((position, directed)) {
                    continue;
                }
                let c = cells.get(&position).copied();
                for (d, &(dr, dc)) in DIRECTIONS.iter().enumerate() {
                    if let Some(c) = c
                        && (!ports(c).contains(&d) || arrow(c).is_some_and(|a| a != d))
                    {
                        continue;
                    }
                    let mut next = (position.0 + dr, position.1 + dc);
                    // Horizontal spacing is cosmetic; vertical and diagonal strokes
                    // must be contiguous to avoid joining unrelated diagram rows.
                    if dr == 0 {
                        while next.1 >= 0
                            && next.1 <= max_col
                            && !cells.contains_key(&next)
                            && !labels.contains_key(&next)
                        {
                            next.1 += dc;
                        }
                    }
                    if let Some((target, target_occurrence)) = labels.get(&next) {
                        if c.is_some() && occurrence != target_occurrence {
                            result
                                .edges
                                .insert((source.clone(), target.clone(), directed));
                        }
                    } else if let Some(&next_c) = cells.get(&next) {
                        let opposite = (d + 4) % 8;
                        if ports(next_c).contains(&opposite) && arrow(next_c).is_none_or(|a| a == d)
                        {
                            pending.push((next, directed || arrow(next_c).is_some()));
                        }
                    }
                }
            }
        }
        Ok(result)
    }
}

pub fn validate(graph: &Graph) -> syn::Result<()> {
    let fail = |message: &str| syn::Error::new_spanned(&graph.module.ident, message);
    let mut docs = String::new();
    for attr in &graph.module.attrs {
        if attr.path().is_ident("doc")
            && let syn::Meta::NameValue(value) = &attr.meta
            && let syn::Expr::Lit(value) = &value.value
            && let syn::Lit::Str(value) = &value.lit
        {
            docs.push_str(&value.value());
            docs.push('\n');
        }
    }
    let mut diagrams = Vec::new();
    let mut block = None;
    let mut other_fence = false;
    for line in docs.lines() {
        let trimmed = line.trim();
        if trimmed == "```" {
            if let Some(text) = block.take() {
                diagrams.push(text);
            } else {
                other_fence = !other_fence;
            }
        } else if trimmed == "```graph" && !other_fence && block.is_none() {
            block = Some(String::new());
        } else if let Some(text) = &mut block {
            text.push_str(line);
            text.push('\n');
        } else if trimmed.starts_with("```") {
            other_fence = !other_fence;
        }
    }
    if block.is_some() {
        return Err(fail("Unclosed ```graph documentation block"));
    }
    if diagrams.is_empty() {
        return Err(fail("Graph modules require a ```graph documentation block"));
    }
    let normalize = |s: String| s.strip_prefix("r#").unwrap_or(&s).to_owned();
    let nodes: BTreeSet<_> = graph
        .functions
        .keys()
        .map(|n| normalize(n.to_string()))
        .collect();
    let mut expected = BTreeSet::new();
    for function in graph.functions.values() {
        for parameter in function.params.borrow().iter() {
            if matches!(parameter.as_ref(), super::function::Parameter::Callee(_)) {
                expected.insert((
                    normalize(parameter.name().to_string()),
                    normalize(function.public_name.to_string()),
                ));
            }
        }
    }
    for text in diagrams {
        let diagram = Diagram::parse(&text).map_err(|message| fail(&message))?;
        if diagram.nodes != nodes {
            let missing: Vec<_> = nodes.difference(&diagram.nodes).collect();
            let unknown: Vec<_> = diagram.nodes.difference(&nodes).collect();
            return Err(fail(&format!(
                "Graph nodes do not match functions: missing {missing:?}, unknown {unknown:?}"
            )));
        }
        let mut actual = BTreeSet::new();
        for (from, to, directed) in diagram.edges {
            let edge = (from, to);
            let reverse = (edge.1.clone(), edge.0.clone());
            if expected.contains(&edge) {
                actual.insert(edge);
            } else if !directed && expected.contains(&reverse) {
                actual.insert(reverse);
            } else {
                return Err(fail(&format!(
                    "Unexpected graph edge: `{}` -> `{}`",
                    edge.0, edge.1
                )));
            }
        }
        if let Some((from, to)) = expected.difference(&actual).next() {
            return Err(fail(&format!("Missing graph edge: `{from}` -> `{to}`")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use quote::quote;

    fn validate(text: &str) -> syn::Result<proc_macro2::TokenStream> {
        super::super::graph2(
            quote! {},
            quote! {
                #[doc = #text]
                mod example {
                    pub fn run(helper: usize, input: usize) -> usize { helper + input }
                    fn helper() -> usize { 1 }
                }
            },
        )
    }

    #[test]
    fn accepts_connectors_and_directions() {
        for diagram in [
            "run <-- helper",
            "helper --> run",
            "run --- helper",
            "run ←── helper",
            "helper ──→ run",
            "run\n |\n helper",
            "run\n │\n ↑\n helper",
            "helper\n ↓\n │\n run",
            "run\n   \\\n    helper",
            "    run\n   /\nhelper",
            "run ─┐\n     │\n     └── helper",
        ] {
            validate(&format!("```graph\n{diagram}\n```"))
                .unwrap_or_else(|error| panic!("{diagram}: {error}"));
        }
    }

    #[test]
    fn rejects_missing_and_incorrect_diagrams() {
        for (text, message) in [
            ("", "require a ```graph"),
            ("```rust\nrun <-- helper\n```", "require a ```graph"),
            ("```graph\nrun <-- helper", "Unclosed"),
            ("```graph\nrun\n```", "missing"),
            ("```graph\nrun <-- helper\nghost\n```", "unknown"),
            ("```graph\nrun    helper\n```", "Missing graph edge"),
            ("```graph\nrun --> helper\n```", "Unexpected graph edge"),
            ("```graph\nrun @ helper\n```", "Unsupported"),
            (
                "```graph\nrun <-- helper\nrun <-- run\n```",
                "Unexpected graph edge",
            ),
            ("```graph\n```", "missing"),
        ] {
            let error = validate(text).unwrap_err();
            assert!(error.to_string().contains(message), "{text}: {error}");
        }
    }

    #[test]
    fn checks_private_chains_isolated_nodes_and_junctions() {
        let diagram =
            "```graph\nrun <-- left <-- leaf\nrun <-- right\nright <-- leaf\nisolated\n```";
        let item = quote! {
            #[doc = #diagram]
            mod example {
                pub fn run(left: usize, right: usize) -> usize { left + right }
                fn left(leaf: usize) -> usize { leaf }
                fn right(leaf: usize) -> usize { leaf }
                fn leaf() -> usize { 1 }
                fn isolated() {}
            }
        };
        super::super::graph2(quote! {}, item).unwrap();
        let parsed = super::Diagram::parse("run --+-- left\n      |\n    right").unwrap();
        assert!(parsed.edges.contains(&("run".into(), "left".into(), false)));
        assert!(
            parsed
                .edges
                .contains(&("run".into(), "right".into(), false))
        );
        assert!(
            !super::Diagram::parse("run <-- left <-- leaf")
                .unwrap()
                .edges
                .contains(&("leaf".into(), "run".into(), true))
        );
    }

    #[test]
    fn directed_fork_and_inner_module_documentation() {
        super::super::graph2(
            quote! {},
            quote! {
                mod example {
                    //! ```graph
                    //! run --+<-- left
                    //!       |
                    //!       └<-- right
                    //! ```
                    pub fn run(left: usize, right: usize) -> usize { left + right }
                    fn left() -> usize { 1 }
                    fn right() -> usize { 2 }
                }
            },
        )
        .unwrap();
    }

    #[test]
    fn unicode_raw_identifiers_and_split_doc_attributes() {
        super::super::graph2(
            quote! {},
            quote! {
                /// ```graph
                /// r#type <-- café
                /// ```
                mod example {
                    pub fn r#type(café: usize) -> usize { café }
                    fn café() -> usize { 1 }
                }
            },
        )
        .unwrap();
    }

    #[test]
    fn validates_every_block() {
        let error =
            validate("```graph\nrun <-- helper\n```\n```graph\nrun helper\n```").unwrap_err();
        assert!(error.to_string().contains("Missing graph edge"));
    }
}
