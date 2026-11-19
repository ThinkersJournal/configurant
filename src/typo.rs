// SPDX-License-Identifier: MIT OR Apache-2.0
//! The nearest known name to a mistyped one, shared by the modules that report an unknown name.

/// The closest of `known` to `unknown` by edit distance, if close enough to be a likely typo.
pub(crate) fn suggestion<'a>(
    unknown: &str,
    known: impl Iterator<Item = &'a str>,
) -> Option<String> {
    let limit = (unknown.chars().count() / 3).max(2);
    known
        .map(|k| (edit_distance(unknown, k), k))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k.to_owned())
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}
