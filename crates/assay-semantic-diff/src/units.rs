//! Syntax-unit extraction and structural matching.
//!
//! Extraction walks a tree-sitter tree and collects named semantic units
//! (functions). Matching compares before/after unit sets and classifies
//! changes into the stable `ChangeKind` categories.

use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::{Node, Tree};

use crate::types::{ChangeKind, Language, SemanticOperation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Unit {
    name: String,
    structure: String,
    position: usize,
}

pub(crate) fn extract_units(language: Language, tree: &Tree, source: &[u8]) -> Vec<Unit> {
    let mut units = Vec::new();
    collect_units(language, tree.root_node(), source, &mut units);
    units.sort_by_key(|unit| unit.position);
    units
}

fn collect_units(language: Language, node: Node<'_>, source: &[u8], units: &mut Vec<Unit>) {
    if node.kind() == language.unit_kind() {
        if let Some(name_node) = node.child_by_field_name("name") {
            units.push(Unit {
                name: String::from_utf8_lossy(&source[name_node.byte_range()]).into_owned(),
                structure: canonical_structure(node, Some(name_node.id()), source),
                position: node.start_byte(),
            });
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_units(language, child, source, units);
    }
}

fn canonical_structure(node: Node<'_>, omitted_node_id: Option<usize>, source: &[u8]) -> String {
    if omitted_node_id == Some(node.id()) {
        return "<declared-name>".to_owned();
    }

    let mut canonical = String::new();
    canonical.push('(');
    canonical.push_str(node.kind());
    let child_count = node.child_count();
    if child_count == 0 {
        if node.is_named() || is_semantic_anonymous_token(node, source) {
            canonical.push(':');
            canonical.push_str(&String::from_utf8_lossy(&source[node.byte_range()]));
        }
    } else {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() || is_semantic_anonymous_token(child, source) {
                canonical.push_str(&canonical_structure(child, omitted_node_id, source));
            }
        }
    }
    canonical.push(')');
    canonical
}

fn is_semantic_anonymous_token(node: Node<'_>, source: &[u8]) -> bool {
    let token = &source[node.byte_range()];
    !matches!(
        token,
        b"{" | b"}" | b"(" | b")" | b"[" | b"]" | b"," | b";" | b":"
    )
}

pub(crate) fn match_units(before: &[Unit], after: &[Unit]) -> Vec<SemanticOperation> {
    let mut operations = Vec::new();
    let before_by_name = unique_names(before);
    let after_by_name = unique_names(after);
    let mut matched_before = BTreeSet::new();
    let mut matched_after = BTreeSet::new();
    let mut unchanged_pairs = Vec::new();

    for (name, &before_index) in &before_by_name {
        let Some(&after_index) = after_by_name.get(name) else {
            continue;
        };
        matched_before.insert(before_index);
        matched_after.insert(after_index);
        if before[before_index].structure == after[after_index].structure {
            unchanged_pairs.push((before_index, after_index));
        } else {
            operations.push(operation(ChangeKind::Modified, Some(name), Some(name)));
        }
    }

    let before_unmatched = unmatched_indices(before.len(), &matched_before);
    let after_unmatched = unmatched_indices(after.len(), &matched_after);
    let mut renamed_before = BTreeSet::new();
    let mut renamed_after = BTreeSet::new();
    for &before_index in &before_unmatched {
        let candidates = after_unmatched
            .iter()
            .copied()
            .filter(|after_index| {
                !renamed_after.contains(after_index)
                    && before[before_index].structure == after[*after_index].structure
            })
            .collect::<Vec<_>>();
        if candidates.len() == 1 {
            let after_index = candidates[0];
            renamed_before.insert(before_index);
            renamed_after.insert(after_index);
            operations.push(operation(
                ChangeKind::Renamed,
                Some(&before[before_index].name),
                Some(&after[after_index].name),
            ));
        }
    }

    unchanged_pairs.sort_unstable();
    let after_sequence = unchanged_pairs
        .iter()
        .map(|(_, after_index)| *after_index)
        .collect::<Vec<_>>();
    let stable_pair_indices = longest_increasing_subsequence_indices(&after_sequence);
    for (pair_index, (before_index, after_index)) in unchanged_pairs.iter().enumerate() {
        if !stable_pair_indices.contains(&pair_index) {
            operations.push(operation(
                ChangeKind::Moved,
                Some(&before[*before_index].name),
                Some(&after[*after_index].name),
            ));
        }
    }

    for before_index in before_unmatched {
        if !renamed_before.contains(&before_index) {
            operations.push(operation(
                ChangeKind::Removed,
                Some(&before[before_index].name),
                None,
            ));
        }
    }
    for after_index in after_unmatched {
        if !renamed_after.contains(&after_index) {
            operations.push(operation(
                ChangeKind::Added,
                None,
                Some(&after[after_index].name),
            ));
        }
    }

    operations.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.before_name.cmp(&right.before_name))
            .then_with(|| left.after_name.cmp(&right.after_name))
    });
    operations
}

fn unique_names(units: &[Unit]) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for unit in units {
        *counts.entry(unit.name.as_str()).or_insert(0_usize) += 1;
    }
    units
        .iter()
        .enumerate()
        .filter(|(_, unit)| counts.get(unit.name.as_str()) == Some(&1))
        .map(|(index, unit)| (unit.name.as_str(), index))
        .collect()
}

fn unmatched_indices(length: usize, matched: &BTreeSet<usize>) -> Vec<usize> {
    (0..length)
        .filter(|index| !matched.contains(index))
        .collect()
}

fn operation(
    kind: ChangeKind,
    before_name: Option<&str>,
    after_name: Option<&str>,
) -> SemanticOperation {
    SemanticOperation {
        kind,
        before_name: before_name.map(str::to_owned),
        after_name: after_name.map(str::to_owned),
    }
}

fn longest_increasing_subsequence_indices(values: &[usize]) -> BTreeSet<usize> {
    let mut lengths = vec![1_usize; values.len()];
    let mut previous = vec![None; values.len()];
    for current in 0..values.len() {
        for prior in 0..current {
            if values[prior] < values[current] && lengths[prior] + 1 > lengths[current] {
                lengths[current] = lengths[prior] + 1;
                previous[current] = Some(prior);
            }
        }
    }
    let Some(mut cursor) = (0..values.len()).max_by_key(|index| lengths[*index]) else {
        return BTreeSet::new();
    };
    let mut stable = BTreeSet::new();
    loop {
        stable.insert(cursor);
        let Some(prior) = previous[cursor] else {
            break;
        };
        cursor = prior;
    }
    stable
}
