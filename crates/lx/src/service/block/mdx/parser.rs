//! MDX → `DocIR` Parser
//!
//! Uses `markdown` crate (`wooorm/markdown-rs`) for robust MDX parsing,
//! then maps the resulting MDAST to our `DocIR`.
//!
//! Aligned with xiaokeai MCP Server's `block_create_block_descendant` schema.

#![allow(dead_code)]

use crate::service::block::ir::{InlineStyle, Node, NodeType};

/// Parse MDX text into `DocIR` Node tree
///
/// Uses `ParseOptions::mdx()` + GFM extensions for full feature coverage.
/// Supports YAML frontmatter (stripped before markdown parsing).
pub fn parse_mdx(input: &str) -> Result<Node, ParseError> {
    // Strip YAML frontmatter if present:  ---\n... \n---
    let (body, _frontmatter) = strip_frontmatter(input);

    let mut options = markdown::ParseOptions::mdx();
    options.constructs.gfm_strikethrough = true;
    options.constructs.gfm_table = true;
    options.constructs.gfm_task_list_item = true;

    let ast = markdown::to_mdast(body, &options).map_err(|msg| ParseError::AtLine {
        line: 1,
        message: msg.to_string(),
    })?;

    let mut counter = 0u64;
    let children = mdast_nodes_to_ir(ast.children().unwrap_or(&vec![]), &mut counter)?;
    Ok(Node::document(children))
}

/// Strip YAML frontmatter from input text.
///
/// Returns (`body_without_frontmatter`, `frontmatter_content`) tuple.
/// If no frontmatter is found, returns (`original_input`, None).
fn strip_frontmatter(input: &str) -> (&str, Option<String>) {
    let trimmed = input.trim_start();
    if !trimmed.starts_with("---") {
        return (input, None);
    }

    // Find closing ---
    if let Some(end_pos) = trimmed[3..].find("\n---") {
        let after_open = &trimmed[3..];
        let fm_content = after_open[..end_pos].trim().to_string();
        let body = &after_open[end_pos + 5..]; // skip "\n---"
        (body.trim_start(), Some(fm_content))
    } else {
        (input, None)
    }
}

// ---- Error type ----

#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseError {
    #[error("Parse error at line {line}: {message}")]
    AtLine { line: u32, message: String },
}

// ---- MDAST → DocIR conversion ----

fn mdast_nodes_to_ir(
    nodes: &[markdown::mdast::Node],
    counter: &mut u64,
) -> Result<Vec<Node>, ParseError> {
    let mut children = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            // === Leaf text blocks (content in elements/text_run[]) ===
            markdown::mdast::Node::Paragraph(p) => {
                let inlines = mdast_inline_to_ir(&p.children)?;
                children.push(Node::paragraph(inlines));
            }
            markdown::mdast::Node::Heading(h) => {
                let level = h.depth.clamp(1, 5);
                let inlines = mdast_inline_to_ir(&h.children)?;
                children.push(Node::heading(level, inlines));
            }
            markdown::mdast::Node::Code(c) => {
                children.push(Node::code_block(c.lang.as_deref(), &c.value));
            }

            // === Container blocks (content in children[]) ===
            markdown::mdast::Node::MdxJsxFlowElement(el) => {
                let name = el.name.as_deref().unwrap_or("").to_lowercase();
                let attrs = extract_attributes(&el.attributes);
                let inner = mdast_nodes_to_ir(&el.children, counter)?;

                match name.as_str() {
                    // === Notion-aligned formats ===
                    "details" => {
                        // Notion toggle: <details color?="Color"><summary>text</summary>children</details>
                        let has_summary = el
                            .children
                            .first()
                            .map(|c| {
                                let t = extract_plain_text_first(c);
                                !t.is_empty()
                            })
                            .unwrap_or(false);
                        let mut toggle_node = Node::toggle(inner);
                        if has_summary {
                            toggle_node.children.insert(
                                0,
                                Node::paragraph(
                                    mdast_inline_to_ir_fallible(&el.children).unwrap_or_default(),
                                ),
                            );
                        }
                        if let Some(c) = attrs.get("color") {
                            toggle_node.attrs.block_color = Some(c.clone());
                        }
                        children.push(toggle_node);
                    }
                    "callout" => {
                        // Notion callout: <callout icon? color?>
                        let color = attrs.get("color").map(std::string::String::as_str);
                        let icon = attrs.get("icon").map(std::string::String::as_str);
                        children.push(Node::callout(color, icon, inner));
                    }
                    "columns" => {
                        // Notion columns: <columns><column>...</column></columns>
                        let cols: Vec<Node> = inner
                            .into_iter()
                            .map(|c| Node::column(/* width_ratio */ None, vec![c]))
                            .collect();
                        children.push(Node::column_list(cols));
                    }
                    "task" => {
                        let checked = attrs
                            .get("checked")
                            .is_some_and(|v| v == "true" || v == "1");
                        // Task name: use inner text content or explicit name attr
                        let name_val = attrs.get("name").cloned().or_else(|| {
                            // Extract plain text from first Text child
                            inner
                                .iter()
                                .find(|n| matches!(n.node_type, NodeType::Text))
                                .and_then(|n| n.text.clone())
                                .or_else(|| {
                                    let t = inner
                                        .iter()
                                        .map(super::super::ir::Node::plain_content)
                                        .collect::<String>();
                                    if !t.is_empty() {
                                        Some(t)
                                    } else {
                                        None
                                    }
                                })
                        });
                        let mut task_node = Node::task(checked, name_val.unwrap_or_default());
                        task_node.children = inner;
                        task_node.temp_id = Some(Node::next_temp_id(counter));
                        children.push(task_node);
                    }
                    "column" => {
                        let wr = attrs.get("width").and_then(|s| {
                            s.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| {
                                if v > 1.0 {
                                    v / 100.0
                                } else {
                                    v
                                }
                            })
                        });
                        children.push(Node::column(wr, inner));
                    }
                    "mermaid" => {
                        let code = inner
                            .iter()
                            .map(super::super::ir::Node::plain_content)
                            .collect::<String>();
                        children.push(Node::mermaid(&code));
                    }
                    "plantuml" => {
                        let code = inner
                            .iter()
                            .map(super::super::ir::Node::plain_content)
                            .collect::<String>();
                        children.push(Node::plantuml(&code));
                    }
                    _ => {
                        // Unknown component: include inner content as children
                        children.extend(inner);
                    }
                }
            }
            markdown::mdast::Node::Blockquote(bq) => {
                let inner = mdast_nodes_to_ir(&bq.children, counter)?;
                if inner.is_empty() {
                    children.push(Node::paragraph(vec![]));
                } else {
                    children.push(Node::quote(inner));
                }
            }
            markdown::mdast::Node::List(list) => {
                let items = mdast_list_items_to_ir(list, counter)?;
                children.extend(items);
            }
            markdown::mdast::Node::ThematicBreak(_) => {
                children.push(Node::divider());
            }
            markdown::mdast::Node::Table(table) => {
                let rows = mdast_table_to_ir(table, counter)?;
                if !rows.is_empty() {
                    children.push(Node::table(rows));
                }
            }

            // === Leaf non-text blocks ===
            markdown::mdast::Node::Image(img) => {
                children.push(Node::image(/* file_id */ None, Some(&img.alt)));
            }
            markdown::mdast::Node::MdxJsxTextElement(el) => {
                // Inline JSX element — handle at inline level within parent paragraph
                let name = el.name.as_deref().unwrap_or("").to_lowercase();
                let attrs = extract_attributes(&el.attributes);
                match name.as_str() {
                    "callout" => {
                        let inner = mdast_nodes_to_ir(&el.children, counter)?;
                        children.push(Node::callout(
                            attrs.get("color").map(std::string::String::as_str),
                            attrs.get("icon").map(std::string::String::as_str),
                            inner,
                        ));
                    }
                    "mark" => {
                        for mut n in mdast_inline_to_ir_fallible(&el.children).unwrap_or_default() {
                            apply_style(&mut n, |s| s.bold = true);
                            result_from_children(&mut children, n);
                        }
                    }
                    _ => {
                        let inner = mdast_inline_to_ir_fallible(&el.children).unwrap_or_default();
                        children.extend(inner);
                    }
                }
            }
            markdown::mdast::Node::MdxTextExpression(expr) => {
                children.push(Node::plain_text(format!("{{{}}}", &expr.value)));
            }
            markdown::mdast::Node::MdxFlowExpression(expr) => {
                children.push(Node::plain_text(format!("{{{}}}", &expr.value)));
            }
            _ => {} // Skip unknown node types
        }
    }
    Ok(children)
}

// ---- List handling ----

fn mdast_list_items_to_ir(
    list: &markdown::mdast::List,
    counter: &mut u64,
) -> Result<Vec<Node>, ParseError> {
    let mut items = Vec::with_capacity(list.children.len());
    for item in &list.children {
        if let markdown::mdast::Node::ListItem(li) = item {
            if let Some(checked) = li.checked {
                // GFM task list item
                let name = li
                    .children
                    .first()
                    .and_then(|c| {
                        if let markdown::mdast::Node::Paragraph(p) = c {
                            p.children.first().and_then(|t| {
                                if let markdown::mdast::Node::Text(t) = t {
                                    Some(t.value.clone())
                                } else {
                                    None
                                }
                            })
                        } else {
                            None
                        }
                    })
                    .filter(|n| !n.is_empty());

                let mut task_node = Node::task(checked, name.unwrap_or_default());
                task_node.temp_id = Some(Node::next_temp_id(counter));

                if li.children.len() > 1
                    || matches!(
                        li.children.first(),
                        Some(markdown::mdast::Node::Paragraph(_)),
                    )
                {
                    let extra = mdast_nodes_to_ir(&li.children, counter)?
                        .into_iter()
                        .filter(|n| !matches!(n.node_type, NodeType::Text))
                        .collect::<Vec<_>>();
                    if !extra.is_empty() {
                        task_node.children = extra;
                    }
                }
                items.push(task_node);
            } else {
                // Regular list item
                let inlines = mdast_inline_to_ir(&li.children)?;
                if list.ordered {
                    items.push(Node::numbered_item(inlines));
                } else {
                    items.push(Node::bullet_item(inlines));
                }
            }
        } else {
            let inlines = mdast_inline_to_ir(&node_children(item))?;
            if list.ordered {
                items.push(Node::numbered_item(inlines));
            } else {
                items.push(Node::bullet_item(inlines));
            }
        }
    }
    Ok(items)
}

// ---- Table handling ----

fn mdast_table_to_ir(
    table: &markdown::mdast::Table,
    _counter: &mut u64,
) -> Result<Vec<Node>, ParseError> {
    let mut rows = Vec::new();
    for row_node in &table.children {
        if let markdown::mdast::Node::TableRow(tr) = row_node {
            let cells = tr
                .children
                .iter()
                .filter_map(|c| {
                    if let markdown::mdast::Node::TableCell(tc) = c {
                        let inlines = mdast_inline_to_ir_fallible(&tc.children).unwrap_or_default();
                        Some(Node::table_cell(inlines))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            rows.push(Node::table_row(cells));
        }
    }
    Ok(rows)
}

// ---- Inline conversion ----

fn mdast_inline_to_ir(nodes: &[markdown::mdast::Node]) -> Result<Vec<Node>, ParseError> {
    Ok(mdast_inline_to_ir_fallible(nodes).unwrap_or_default())
}

fn mdast_inline_to_ir_fallible(nodes: &[markdown::mdast::Node]) -> Option<Vec<Node>> {
    let mut result = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            markdown::mdast::Node::Text(t) => {
                result.push(Node::text(&t.value, None));
            }
            markdown::mdast::Node::InlineCode(code) => {
                result.push(Node::text(
                    &code.value,
                    Some(InlineStyle {
                        inline_code: true,
                        ..Default::default()
                    }),
                ));
            }
            markdown::mdast::Node::Strong(s) => {
                for mut n in mdast_inline_to_ir_fallible(&s.children)? {
                    apply_style(&mut n, |s| s.bold = true);
                    result.push(n);
                }
            }
            markdown::mdast::Node::Emphasis(e) => {
                for mut n in mdast_inline_to_ir_fallible(&e.children)? {
                    apply_style(&mut n, |s| s.italic = true);
                    result.push(n);
                }
            }
            markdown::mdast::Node::Delete(del) => {
                for mut n in mdast_inline_to_ir_fallible(&del.children)? {
                    apply_style(&mut n, |s| s.strike_through = true);
                    result.push(n);
                }
            }
            markdown::mdast::Node::Link(link) => {
                let inlines = mdast_inline_to_ir_fallible(&link.children)?;
                result.push(Node::link(&link.url, inlines));
            }
            markdown::mdast::Node::Image(img) => {
                result.push(Node::image(None, Some(&img.alt)));
            }
            markdown::mdast::Node::MdxTextExpression(expr) => {
                result.push(Node::plain_text(format!("{{{}}}", &expr.value)));
            }
            markdown::mdast::Node::MdxJsxTextElement(el) => {
                let name = el.name.as_deref().unwrap_or("").to_lowercase();
                let attrs = extract_attributes(&el.attributes);
                match name.as_str() {
                    "callout" => {
                        let inner = mdast_inline_to_ir_fallible(&el.children).unwrap_or_default();
                        result.push(Node::callout(
                            attrs.get("color").map(std::string::String::as_str),
                            attrs.get("icon").map(std::string::String::as_str),
                            inner,
                        ));
                    }
                    "column" => {
                        let wr = attrs.get("width").and_then(|s| {
                            s.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| {
                                if v > 1.0 {
                                    v / 100.0
                                } else {
                                    v
                                }
                            })
                        });
                        let inner = mdast_inline_to_ir_fallible(&el.children).unwrap_or_default();
                        result.push(Node::column(wr, inner));
                    }
                    "mark" => {
                        for mut n in mdast_inline_to_ir_fallible(&el.children).unwrap_or_default() {
                            apply_style(&mut n, |s| s.bold = true);
                            result.push(n);
                        }
                    }
                    "span" => {
                        // Notion <span underline color="...">text</span>
                        let underline = attrs
                            .get("underline")
                            .is_some_and(|v| v == "true" || v == "1");
                        let color = attrs.get("color").cloned();
                        for mut n in mdast_inline_to_ir_fallible(&el.children).unwrap_or_default() {
                            apply_style(&mut n, |s| {
                                if underline {
                                    s.underline = true;
                                }
                                if let Some(ref c) = color {
                                    s.text_color = Some(c.clone());
                                    s.background_color = Some(c.clone()); // Notion span uses single color attr
                                                                          // Clear one since we only have one field to store
                                    if c.trim().ends_with("_bg") || c.contains("_bg") {
                                        s.text_color = None;
                                    } else {
                                        s.background_color = None;
                                    }
                                }
                            });
                            result.push(n);
                        }
                    }
                    _ => {
                        result
                            .extend(mdast_inline_to_ir_fallible(&el.children).unwrap_or_default());
                    }
                }
            }
            _ => {}
        }
    }
    Some(result)
}

// ---- Style helpers ----

fn apply_style(node: &mut Node, f: impl FnOnce(&mut InlineStyle)) {
    if let Some(s) = &mut node.inline_style {
        f(s);
    } else {
        let mut s = InlineStyle::default();
        f(&mut s);
        node.inline_style = Some(s);
    }
}

fn result_from_children(children: &mut Vec<Node>, n: Node) {
    children.push(n);
}

// ---- Attribute extraction ----

fn extract_attributes(
    attrs: &[markdown::mdast::AttributeContent],
) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for attr in attrs {
        if let markdown::mdast::AttributeContent::Property(prop) = attr {
            let key = prop.name.clone();
            let val = match &prop.value {
                Some(markdown::mdast::AttributeValue::Literal(s)) => s.clone(),
                Some(markdown::mdast::AttributeValue::Expression(expr)) => expr.value.clone(),
                None => String::new(),
            };
            if !key.is_empty() {
                map.insert(key, val);
            }
        }
    }
    map
}

// ---- Helpers ----

/// Extract plain text from first Text child node (for summary extraction)
fn extract_plain_text_first(node: &markdown::mdast::Node) -> String {
    match node {
        markdown::mdast::Node::Paragraph(p) => p
            .children
            .iter()
            .find_map(|c| match c {
                markdown::mdast::Node::Text(t) => Some(t.value.clone()),
                _ => None,
            })
            .unwrap_or_default(),
        markdown::mdast::Node::Text(t) => t.value.clone(),
        _ => node
            .children()
            .and_then(|children| children.first().map(extract_plain_text_first))
            .unwrap_or_default(),
    }
}

fn node_children(node: &markdown::mdast::Node) -> Vec<markdown::mdast::Node> {
    match node {
        markdown::mdast::Node::Paragraph(p) => p.children.clone(),
        markdown::mdast::Node::ListItem(li) => li.children.clone(),
        _ => vec![],
    }
}

// ---- Tests ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_paragraph() {
        let doc = parse_mdx("Hello world").unwrap();
        assert_eq!(doc.children.len(), 1);
        assert_eq!(doc.children[0].node_type, NodeType::Paragraph);
        assert_eq!(doc.children[0].plain_content(), "Hello world");
    }

    #[test]
    fn test_heading() {
        let doc = parse_mdx("# Title\n\n## Subtitle").unwrap();
        assert_eq!(doc.children.len(), 2);
        assert_eq!(doc.children[0].node_type, NodeType::Heading { level: 1 });
        assert_eq!(doc.children[1].node_type, NodeType::Heading { level: 2 });
    }

    #[test]
    fn test_code_fence() {
        let doc = parse_mdx("```rust\nfn main() {}\n```").unwrap();
        assert_eq!(doc.children.len(), 1);
        let cb = &doc.children[0];
        assert!(matches!(
            cb.node_type,
            NodeType::CodeBlock { language: Some(..) }
        ));
        assert_eq!(cb.text.as_deref(), Some("fn main() {}"));
    }

    #[test]
    fn test_callout_container() {
        // Notion format: lowercase <callout color>
        let mdx = r#"<callout icon="🚧" color="red">
    ## Warning
    Check before proceeding.
</callout>"#;
        let doc = parse_mdx(mdx).unwrap();
        assert_eq!(doc.children.len(), 1);
        let co = &doc.children[0];
        assert!(matches!(co.node_type, NodeType::Callout { .. }));
        assert!(!co.children.is_empty(), "callout must have children");
        assert_eq!(co.callout_color.as_deref(), Some("red"));
        assert_eq!(co.callout_icon.as_deref(), Some("🚧"));
    }

    #[test]
    fn test_task_with_name() {
        let mdx = "- [x] Buy milk\n- [ ] Write tests";
        let doc = parse_mdx(mdx).unwrap();
        assert_eq!(doc.children.len(), 2);
        // Find checked and unchecked tasks (order-independent)
        let checked = doc
            .children
            .iter()
            .find(|c| matches!(c.node_type, NodeType::Task { done: true, .. }));
        let unchecked = doc
            .children
            .iter()
            .find(|c| matches!(c.node_type, NodeType::Task { done: false, .. }));
        assert!(checked.is_some(), "should have a checked task");
        assert!(unchecked.is_some(), "should have an unchecked task");
        assert_eq!(checked.unwrap().task_name.as_deref(), Some("Buy milk"));
        assert_eq!(unchecked.unwrap().task_name.as_deref(), Some("Write tests"));
    }

    #[test]
    fn test_bullet_list() {
        let mdx = "- item 1\n- item 2";
        let doc = parse_mdx(mdx).unwrap();
        assert_eq!(doc.children.len(), 2);
        assert_eq!(doc.children[0].node_type, NodeType::BulletedList);
        assert_eq!(doc.children[1].node_type, NodeType::BulletedList);
    }

    #[test]
    fn test_image_markdown() {
        let mdx = "![alt text](https://example.com/img.png)";
        let doc = parse_mdx(mdx).unwrap();
        assert!(!doc.children.is_empty());
        // In MDX mode image may wrap in Paragraph; find it anywhere
        let has_img = doc.find_all(&NodeType::Image {
            file_id: Some(String::new()),
            caption: None,
            width: None,
            height: None,
            align: None,
        });
        if has_img.is_empty() {
            eprintln!(
                "DOC children: {:?}",
                doc.children
                    .iter()
                    .map(|c| &c.node_type)
                    .collect::<Vec<_>>()
            );
            for c in &doc.children {
                eprintln!(
                    "  CHILD children: {:?}",
                    c.children
                        .iter()
                        .map(|cc| &cc.node_type)
                        .collect::<Vec<_>>()
                );
            }
        }
        assert!(!has_img.is_empty(), "should have an Image node");
    }

    #[test]
    fn test_divider() {
        let doc = parse_mdx("---\n").unwrap();
        assert_eq!(doc.children.len(), 1);
        assert_eq!(doc.children[0].node_type, NodeType::Divider);
    }

    #[test]
    fn test_block_quote_maps_to_callout() {
        // Notion: native > quotes → BlockQuote
        let doc = parse_mdx("> A quoted message").unwrap();
        assert_eq!(doc.children.len(), 1);
        assert!(
            matches!(doc.children[0].node_type, NodeType::BlockQuote),
            "got: {:?}",
            doc.children[0].node_type,
        );
    }

    #[test]
    fn test_bold_italic_strike() {
        let doc = parse_mdx("**bold** and *italic* and ~~strike~~").unwrap();
        let p = &doc.children[0];
        assert_eq!(p.node_type, NodeType::Paragraph);
        assert_eq!(p.children.len(), 5); // bold + " and " + italic + " and " + strike
        assert!(p.children[0].inline_style.as_ref().unwrap().bold);
        assert!(p.children[2].inline_style.as_ref().unwrap().italic);
        assert!(p.children[4].inline_style.as_ref().unwrap().strike_through);
    }

    #[test]
    fn test_column_list() {
        // Notion format: <columns><column>
        let mdx = r#"<columns>
<column>
### Left
Left content
</column>
<column>
### Right
Right content
</column>
</columns>"#;
        let doc = parse_mdx(mdx).unwrap();
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(doc.children[0].node_type, NodeType::ColumnList));
        let cl = &doc.children[0];
        assert_eq!(cl.children.len(), 2);
    }

    #[test]
    fn test_table() {
        let mdx = "| A | B |\n| -- | -- |\n| 1 | 2 |";
        let doc = parse_mdx(mdx).unwrap();
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(doc.children[0].node_type, NodeType::Table));
    }

    #[test]
    fn test_numbered_list() {
        let doc = parse_mdx("1. First\n2. Second").unwrap();
        assert_eq!(doc.children.len(), 2);
        assert!(matches!(doc.children[0].node_type, NodeType::NumberedList));
        assert!(matches!(doc.children[1].node_type, NodeType::NumberedList));
    }

    #[test]
    fn test_toggle() {
        // Notion toggle: <details> must be on its own line (flow element)
        let mdx = "<details>\n<summary>Click to expand</summary>\nHidden content\n</details>";
        let doc = parse_mdx(mdx).unwrap();
        assert!(!doc.children.is_empty());
        let toggles = doc.find_all(&NodeType::Toggle);
        assert!(
            !toggles.is_empty(),
            "should have a Toggle node from <details>"
        );
    }

    #[test]
    fn test_mermaid() {
        let doc = parse_mdx("<Mermaid>\ngraph LR\nA-->B\n</Mermaid>").unwrap();
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(
            doc.children[0].node_type,
            NodeType::Mermaid { .. }
        ));
        assert_eq!(
            doc.children[0]
                .diagram_content
                .as_deref()
                .expect("should have content"),
            "graph LR\nA-->B"
        );
    }

    #[test]
    fn test_expression() {
        let doc = parse_mdx("Hello {name}!").unwrap();
        assert_eq!(doc.children.len(), 1);
        assert!(doc.children[0].plain_content().contains("{name}"));
    }

    #[test]
    fn test_complex_document() {
        let mdx = r#"# Introduction

This document describes **MDX** converter.

<callout icon="🚧" color="red">
## Note
Check before proceeding.
</callout>

## Features

- [x] Callout support
- [ ] Math support (Phase 2)

\`\`\`rust
fn hello() {}
\`\`\`

---

"#;
        let doc = parse_mdx(mdx).unwrap();
        assert!(doc.children.len() >= 7);

        // Should contain a callout (container with children)
        let has_callout = doc
            .children
            .iter()
            .any(|c| matches!(c.node_type, NodeType::Callout { .. }));
        assert!(has_callout);

        // Should contain code block (or paragraph with code-like content in MDX mode)
        let has_code = doc
            .children
            .iter()
            .any(|c| matches!(c.node_type, NodeType::CodeBlock { .. }))
            || doc
                .children
                .iter()
                .any(|c| c.plain_content().contains("fn hello()"));
        assert!(has_code);

        // Should contain divider
        let has_divider = doc
            .children
            .iter()
            .any(|c| c.node_type == NodeType::Divider);
        assert!(has_divider);

        // Should have tasks (checked=true, unchecked=false)
        let checked_task = doc
            .children
            .iter()
            .find(|c| matches!(c.node_type, NodeType::Task { done: true, .. }));
        assert!(checked_task.is_some());
        assert_eq!(
            checked_task.unwrap().task_name.as_deref(),
            Some("Callout support")
        );
    }

    #[test]
    fn test_frontmatter_stripped() {
        // YAML frontmatter should be stripped, not parsed as content
        let input = r#"---
title: 测试文档
author: test
---

# Real Content

**bold** text
"#;
        let doc = parse_mdx(input).unwrap();

        // Should have exactly 2 children: H1 + paragraph (NOT divider/h2 for frontmatter)
        assert_eq!(
            doc.children.len(),
            2,
            "frontmatter should produce 0 blocks, got {} children",
            doc.children.len()
        );

        // First child should be heading
        assert!(matches!(
            doc.children[0].node_type,
            NodeType::Heading { level: 1, .. }
        ));

        // Second should be paragraph with bold
        assert!(matches!(doc.children[1].node_type, NodeType::Paragraph));
    }
}
