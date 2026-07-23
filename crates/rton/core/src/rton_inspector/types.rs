#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtonTagInfo {
    pub name: &'static str,
    pub category: &'static str,
    pub payload_kind: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtonPayloadInfo {
    pub label: String,
    pub value: String,
    pub bytes: Option<String>,
    pub range: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtonVarintInfo {
    pub value: String,
    pub zigzag: String,
    pub length: usize,
    pub bytes: String,
    pub next_offset: usize,
    pub raw_value: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtonStringMode {
    Direct,
    Definition,
    Reference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtonStringInfo {
    pub mode: RtonStringMode,
    pub encoding: &'static str,
    pub table: Option<&'static str>,
    pub index: Option<String>,
    pub length: Option<String>,
    pub byte_length: Option<String>,
    pub text: Option<String>,
    pub resolved_text: Option<String>,
    pub scan_limited: bool,
    pub scan_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtonStringTables {
    pub ascii: Vec<String>,
    pub utf8: Vec<String>,
    pub limited: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtonAsciiRun {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedStringPayload {
    pub(crate) text: String,
    pub(crate) length: Option<u128>,
    pub(crate) byte_length: u128,
    pub(crate) end_offset: usize,
}
