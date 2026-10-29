// SPDX-License-Identifier: MIT OR Apache-2.0
//! The README and hub-card example (`examples/quickstart.rs`) is run here, so it cannot go stale.
#![allow(dead_code)]

include!("../examples/quickstart.rs");

#[test]
fn the_quickstart_example_runs() {
    main();
}

/// The README's first code block must be the example, line for line (minus the SPDX header).
#[test]
fn the_readme_snippet_is_the_example() {
    let readme = include_str!("../README.md").replace("\r\n", "\n");
    let start = readme
        .find("```rust\n")
        .expect("README opens with a rust block")
        + 8;
    let end = start + readme[start..].find("```").expect("block closes");
    let snippet = readme[start..end].trim_end();
    let example = include_str!("../examples/quickstart.rs").replace("\r\n", "\n");
    let body = example.split_once('\n').expect("SPDX line").1.trim_end();
    assert_eq!(
        snippet, body,
        "README hello-world drifted from examples/quickstart.rs"
    );
}
