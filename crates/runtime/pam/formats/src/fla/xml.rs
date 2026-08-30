use std::collections::BTreeMap;

pub(super) const XFL_NS: &str = "http://ns.adobe.com/xfl/2008/";
pub(super) const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

#[derive(Default)]
pub(super) struct XmlBuilder {
    parts: Vec<String>,
    indent: usize,
}

impl XmlBuilder {
    pub fn open<'a>(&mut self, tag: &str, attrs: impl IntoIterator<Item = (&'a str, String)>) {
        let mut line = format!("{}<{tag}", "\t".repeat(self.indent));
        for (name, value) in attrs {
            line.push_str(&format!(" {name}=\"{}\"", escape_xml(&value)));
        }
        line.push('>');
        self.parts.push(line);
        self.indent += 1;
    }

    pub fn close(&mut self, tag: &str) {
        self.indent = self.indent.saturating_sub(1);
        self.parts
            .push(format!("{}</{tag}>", "\t".repeat(self.indent)));
    }

    pub fn empty<'a>(&mut self, tag: &str, attrs: impl IntoIterator<Item = (&'a str, String)>) {
        let mut line = format!("{}<{tag}", "\t".repeat(self.indent));
        for (name, value) in attrs {
            line.push_str(&format!(" {name}=\"{}\"", escape_xml(&value)));
        }
        line.push_str("/>");
        self.parts.push(line);
    }

    pub fn raw(&mut self, value: impl Into<String>) {
        self.parts.push(value.into());
    }

    pub fn finish(self) -> String {
        self.parts.join("\n")
    }
}

pub(super) fn attrs<const N: usize>(values: [(&str, String); N]) -> BTreeMap<&str, String> {
    values.into_iter().collect()
}

pub(super) fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(super) fn local_name<'a, 'input>(node: roxmltree::Node<'a, 'input>) -> &'input str {
    node.tag_name().name()
}

pub(super) fn descendants_named<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = roxmltree::Node<'a, 'input>> {
    node.descendants()
        .filter(move |child| child.is_element() && local_name(*child) == name)
}

pub(super) fn attr_f64(node: roxmltree::Node<'_, '_>, name: &str, default: f64) -> f64 {
    node.attribute(name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

pub(super) fn attr_usize(node: roxmltree::Node<'_, '_>, name: &str, default: usize) -> usize {
    node.attribute(name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}
