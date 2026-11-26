//! The two characters of a project tile: the rule of the section "Label of
//! a project tile" of the sidebar specification.

use std::collections::HashMap;

use nomux_client::{NodeKind, WorkspaceTree};

/// The label of each Project of the tree, by node identifier.
#[must_use]
pub(crate) fn project_labels(tree: &WorkspaceTree) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    let mut parents = vec![None];
    parents.extend(
        tree.nodes()
            .into_iter()
            .filter(|node| node.is_group())
            .map(|node| Some(node.id.as_str())),
    );
    for parent in parents {
        label_siblings(tree, parent, &mut labels);
    }
    labels
}

/// Gives a label to each Project in the Group `parent`.
fn label_siblings(
    tree: &WorkspaceTree,
    parent: Option<&str>,
    labels: &mut HashMap<String, String>,
) {
    let projects: Vec<(&str, Vec<String>)> = tree
        .children(parent)
        .iter()
        .filter_map(|id| tree.get(id))
        .filter(|node| matches!(node.kind, NodeKind::Project { .. }))
        .map(|node| (node.id.as_str(), words(&node.name)))
        .collect();
    let shared = if projects.len() > 1 {
        shared_leading_words(projects.iter().map(|(_, words)| words.as_slice()))
    } else {
        0
    };
    let group_words = ancestor_group_words(tree, parent);
    let mut used: Vec<String> = Vec::new();
    for (id, words) in &projects {
        let remaining = remaining_words(words, shared, &group_words);
        let label = free_label(&remaining, &used);
        used.push(label.clone());
        labels.insert((*id).to_owned(), label);
    }
}

/// The words of a name: separated by `-`, `_`, `.` and spaces.
fn words(name: &str) -> Vec<String> {
    name.split(['-', '_', '.', ' '])
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The number of leading words that all lists have.
fn shared_leading_words<'a>(lists: impl Iterator<Item = &'a [String]> + Clone) -> usize {
    let shortest = lists.clone().map(<[String]>::len).min().unwrap_or(0);
    (0..shortest)
        .take_while(|&index| {
            let mut words = lists.clone().map(|list| list[index].to_lowercase());
            let first = words.next();
            words.all(|word| Some(word) == first)
        })
        .count()
}

/// The words of the names of the Groups that hold `parent`, `parent`
/// included, in lower case.
fn ancestor_group_words(tree: &WorkspaceTree, parent: Option<&str>) -> Vec<String> {
    let mut group_words = Vec::new();
    let mut current = parent;
    while let Some(node) = current.and_then(|id| tree.get(id)) {
        group_words.extend(words(&node.name).iter().map(|word| word.to_lowercase()));
        current = node.parent_id.as_deref();
    }
    group_words
}

/// The words of a name without the shared leading words and without the
/// words of a Group name. If no word stays, the first word after the shared
/// words stays.
fn remaining_words(words: &[String], shared: usize, group_words: &[String]) -> Vec<String> {
    let after_shared = &words[shared.min(words.len())..];
    let remaining: Vec<String> = after_shared
        .iter()
        .filter(|word| !group_words.contains(&word.to_lowercase()))
        .cloned()
        .collect();
    if remaining.is_empty() {
        after_shared
            .first()
            .or(words.first())
            .cloned()
            .into_iter()
            .collect()
    } else {
        remaining
    }
}

/// One word: its first letter in upper case and its second in lower case.
/// Two words or more: the first letters of the first two words in upper
/// case. If an earlier project has the label, the second character is the
/// next letter of the words that gives a free label.
fn free_label(words: &[String], used: &[String]) -> String {
    let letters: Vec<char> = words.iter().flat_map(|word| word.chars()).collect();
    let Some(first) = letters.first() else {
        return String::new();
    };
    let first: String = first.to_uppercase().collect();
    let single_word = words.len() == 1;
    let second_of = |letter: char| -> String {
        if single_word {
            letter.to_lowercase().collect()
        } else {
            letter.to_uppercase().collect()
        }
    };
    let preferred = if single_word {
        letters.get(1).copied()
    } else {
        words.get(1).and_then(|word| word.chars().next())
    };
    let candidates = preferred.into_iter().chain(letters.iter().skip(1).copied());
    let mut fallback = None;
    for letter in candidates {
        let label = format!("{first}{}", second_of(letter));
        if !used.contains(&label) {
            return label;
        }
        fallback.get_or_insert(label);
    }
    fallback.unwrap_or(first)
}
