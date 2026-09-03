/// The reference goes stale the first week nobody checks it. This makes the
/// test suite the thing that checks it.
#[test]
fn every_tool_has_a_heading_in_docs_mcp_md() {
    let doc = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/mcp.md"),
    )
    .expect("docs/mcp.md should exist");
    for name in ambient_context_lib::mcp::tools::defs() {
        assert!(
            doc.contains(&format!("### `{}`", name.name)),
            "docs/mcp.md has no section for {}",
            name.name
        );
    }
}

#[test]
fn the_document_does_not_describe_a_tool_that_does_not_exist() {
    let doc = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/mcp.md"),
    )
    .unwrap();
    for line in doc.lines().filter(|line| line.starts_with("### `")) {
        let name = line.trim_start_matches("### `").trim_end_matches('`');
        assert!(
            ambient_context_lib::mcp::tools::exists(name),
            "docs/mcp.md documents {name}, which is not a tool"
        );
    }
}
