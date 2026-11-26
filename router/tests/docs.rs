#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Golem examples in the documentation are parsed by the product's own parser.

use router::parse_golem;

const DOCUMENTS: [(&str, &str); 4] = [
    ("README.md", include_str!("../../README.md")),
    ("SPEC.md", include_str!("../../SPEC.md")),
    ("OPERATIONS.md", include_str!("../../OPERATIONS.md")),
    ("book/src/write.md", include_str!("../../book/src/write.md")),
];

/// Every `markdown` code block that opens with frontmatter, named after the closest
/// preceding `.golems/<name>.md` mention.
fn golem_examples(document: &str) -> Vec<(String, String)> {
    let mut examples = Vec::new();
    let mut name: Option<String> = None;
    let mut block: Option<String> = None;
    for line in document.lines() {
        if let Some(body) = block.as_mut() {
            if line == "```" {
                let body = block.take().unwrap_or_default();
                if body.starts_with("---\n") {
                    let name = name
                        .clone()
                        .expect("golem example without a .golems/<name>.md mention before it");
                    examples.push((name, body));
                }
            } else {
                body.push_str(line);
                body.push('\n');
            }
            continue;
        }
        if line == "```markdown" {
            block = Some(String::new());
            continue;
        }
        let mentioned = line
            .split_once(".golems/")
            .and_then(|(_, rest)| rest.split_once(".md"))
            .map(|(candidate, _)| candidate);
        if let Some(candidate) = mentioned
            && !candidate.is_empty()
            && candidate
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            name = Some(candidate.to_owned());
        }
    }
    assert!(block.is_none(), "unterminated code block");
    examples
}

#[test]
fn golem_examples_in_the_documentation_parse_without_errors() {
    for (file, document) in DOCUMENTS {
        let examples = golem_examples(document);
        if matches!(file, "README.md" | "book/src/write.md") {
            assert!(!examples.is_empty(), "{file} must show at least one golem");
        }
        for (name, body) in examples {
            let golem = parse_golem(&name, &body);
            assert!(
                golem.errors.is_empty(),
                "{file}: example `{name}` is invalid: {:?}",
                golem.errors
            );
        }
    }
}

#[test]
fn an_invalid_example_is_reported() {
    let document = "`.golems/broken.md`:\n\n```markdown\n---\nharness: nonsense\nunknown: field\n---\nTask\n```\n";
    let examples = golem_examples(document);
    assert_eq!(examples.len(), 1);
    let (name, body) = examples.first().unwrap();
    assert_eq!(name, "broken");
    assert_ne!(parse_golem(name, body).errors, Vec::<String>::new());
}
