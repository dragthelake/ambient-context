// Replaced entirely in Task 7
pub fn list() -> Vec<serde_json::Value> {
    Vec::new()
}
pub fn exists(_name: &str) -> bool {
    false
}
pub fn call(
    _server: &mut crate::mcp::Server,
    _name: &str,
    _arguments: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "content": [{ "type": "text", "text": "not yet implemented" }],
        "isError": true
    })
}
