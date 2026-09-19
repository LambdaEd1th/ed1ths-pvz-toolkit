//! Loss-conscious resource-definition edits. Unknown JSON/RTON fields and
//! untouched RTON values are retained; slots are stable across renames/deletes.
use crate::resources::{ResourceEntry, normalize_path};
use serde_json::{Value, json};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    fn entry(value: &Value, id: &str) -> ResourceEntry {
        crate::resources::parse_manifest_json("resources.rton", value.clone())
            .unwrap()
            .resources
            .into_iter()
            .find(|r| r.id == id)
            .unwrap()
    }

    #[test]
    fn inherited_group_context_and_array_paths_are_preserved() {
        let mut value = crate::edit_fixture::manifest();
        value["groups"][1].as_object_mut().unwrap().remove("res");
        value["groups"][1].as_object_mut().unwrap().remove("parent");
        value["groups"][1]["resources"][1]["path"] = json!(["images", "leaf"]);
        let before = entry(&value, "LEAF");
        let mut after = before.clone();
        after.path = "images/new_leaf".into();
        let mut document =
            ManifestDocument::decode("resources.json", &serde_json::to_vec(&value).unwrap())
                .unwrap();
        assert_eq!(document.apply(Some(&before), Some(&after), 3).unwrap(), 1);
        assert_eq!(
            document.value["groups"][1]["resources"][1]["path"],
            json!(["images", "new_leaf"])
        );
        assert!(document.value["groups"][1]["res"].is_null());
    }

    #[test]
    fn moving_a_resource_retains_opaque_rton_types_and_slot() {
        use serde_rton::Value as R;
        let value = crate::edit_fixture::manifest();
        let before = entry(&value, "CONFIG");
        let mut after = before.clone();
        after.group = "New".into();
        after.subgroup = "New".into();
        let mut original = new_rton(&value);
        if let R::Object(fields) = &mut original {
            let R::Array(groups) = &mut fields.iter_mut().find(|(k, _)| k == "groups").unwrap().1
            else {
                panic!()
            };
            let R::Object(fields) = &mut groups[2] else {
                panic!()
            };
            let R::Array(items) = &mut fields.iter_mut().find(|(k, _)| k == "resources").unwrap().1
            else {
                panic!()
            };
            let R::Object(fields) = &mut items[0] else {
                panic!()
            };
            fields.push(("opaque".into(), R::Int8(-8)));
        }
        let bytes = serde_rton::to_bytes(&original).unwrap();
        let mut document = ManifestDocument::decode("resources.rton", &bytes).unwrap();
        document.apply(Some(&before), Some(&after), 3).unwrap();
        let encoded: R = serde_rton::from_bytes(&document.encode().unwrap()).unwrap();
        let R::Array(groups) = rton_field(&encoded, "groups").unwrap() else {
            panic!()
        };
        let R::Array(items) = rton_field(groups.last().unwrap(), "resources").unwrap() else {
            panic!()
        };
        assert_eq!(rton_field(&items[0], "opaque"), Some(&R::Int8(-8)));
        assert_eq!(serde_json::to_value(&items[0]).unwrap()["slot"], 2);
    }

    #[test]
    fn compact_rton_edits_keep_the_compact_encoding() {
        let value = crate::edit_fixture::manifest();
        let before = entry(&value, "CONFIG");
        let mut after = before.clone();
        after.id = "NEW_CONFIG".into();
        let bytes = serde_rton::to_compact_bytes(&new_rton(&value)).unwrap();
        let mut document = ManifestDocument::decode("resources.rton", &bytes).unwrap();
        document.apply(Some(&before), Some(&after), 3).unwrap();
        let bytes = document.encode().unwrap();
        assert_eq!(
            &bytes[4..8],
            &serde_rton::tags::COMPACT_FILE_VERSION.to_le_bytes()
        );
        assert_eq!(
            entry(
                &ManifestDocument::decode("resources.rton", &bytes)
                    .unwrap()
                    .value,
                "NEW_CONFIG"
            )
            .path,
            before.path
        );
    }
}

enum Encoding {
    Json,
    Newton,
    Rton {
        compact: bool,
        original: serde_rton::Value,
    },
}

pub struct ManifestDocument {
    pub value: Value,
    encoding: Encoding,
}

impl ManifestDocument {
    pub fn decode(name: &str, bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 128 * 1024 * 1024 {
            return Err("资源清单超过 128 MiB".into());
        }
        let (value, encoding) = if bytes.starts_with(b"RTON") {
            let original: serde_rton::Value =
                serde_rton::from_bytes(bytes).map_err(|e| e.to_string())?;
            let value = serde_json::to_value(&original).map_err(|e| e.to_string())?;
            let compact = bytes.get(4..8)
                == Some(&serde_rton::tags::COMPACT_FILE_VERSION.to_le_bytes())
                || bytes.get(8) == Some(&(serde_rton::tags::RtonTag::CompactObjectBegin as u8));
            (value, Encoding::Rton { compact, original })
        } else if name.to_ascii_lowercase().ends_with(".newton") {
            let value = newton_manifest::from_bytes(bytes).map_err(|e| e.to_string())?;
            (
                serde_json::to_value(value).map_err(|e| e.to_string())?,
                Encoding::Newton,
            )
        } else {
            (
                serde_json::from_slice(bytes).map_err(|e| e.to_string())?,
                Encoding::Json,
            )
        };
        if !value["groups"].is_array() && !value["groups"].is_object() {
            return Err("清单缺少 groups".into());
        }
        Ok(Self { value, encoding })
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        match &self.encoding {
            Encoding::Json => serde_json::to_vec_pretty(&self.value).map_err(|e| e.to_string()),
            Encoding::Newton => {
                let value = serde_json::from_value(self.value.clone())
                    .map_err(|e| format!("NEWTON 字段无效：{e}"))?;
                newton_manifest::to_bytes(&value).map_err(|e| e.to_string())
            }
            Encoding::Rton { compact, original } => {
                let old = serde_json::to_value(original).map_err(|e| e.to_string())?;
                let value = reconcile_rton(original, &old, &self.value);
                if *compact {
                    serde_rton::to_compact_bytes(&value)
                } else {
                    serde_rton::to_bytes(&value)
                }
                .map_err(|e| e.to_string())
            }
        }
    }

    pub fn apply(
        &mut self,
        before: Option<&ResourceEntry>,
        after: Option<&ResourceEntry>,
        slot: u32,
    ) -> Result<usize, String> {
        if self.value["groups"].is_object() {
            return apply_description(&mut self.value, before, after);
        }
        let count = apply_groups(&mut self.value, before, after, slot)?;
        if count != 0
            && matches!(self.encoding, Encoding::Newton)
            && after.is_some_and(|e| !e.language.is_empty())
        {
            return Err("NEWTON 不支持 locale 字段，不能无损同步这个语言值".into());
        }
        Ok(count)
    }
}

pub fn text(value: &Value, key: &str) -> String {
    match &value[key] {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

pub fn next_slot(value: &Value) -> u32 {
    let max = value["groups"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|g| g["resources"].as_array().into_iter().flatten())
        .filter_map(|r| r["slot"].as_u64())
        .max()
        .map_or(0, |n| n.saturating_add(1));
    value["slot_count"]
        .as_u64()
        .unwrap_or(0)
        .max(max)
        .min(u32::MAX as u64) as u32
}

fn kind_number(kind: &str) -> Option<u32> {
    [
        "Image",
        "PopAnim",
        "SoundBank",
        "File",
        "PrimeFont",
        "RenderEffect",
        "DecodedSoundBank",
    ]
    .iter()
    .position(|k| *k == kind)
    .map(|i| i as u32 + 1)
    .or_else(|| kind.strip_prefix("Type ")?.parse().ok())
}

fn set_path(item: &mut Value, path: &str) {
    item["path"] = if item["path"].is_array() {
        json!(path.split(['/', '\\']).collect::<Vec<_>>())
    } else {
        json!(path)
    };
}

fn patch_item(
    item: &mut Value,
    before: Option<&ResourceEntry>,
    after: &ResourceEntry,
) -> Result<(), String> {
    let changed =
        |field: fn(&ResourceEntry) -> &String| before.is_none_or(|old| field(old) != field(after));
    item["id"] = json!(after.id);
    if changed(|e| &e.path) {
        set_path(item, &after.path);
    }
    if changed(|e| &e.kind) {
        item["type"] = if item["type"].is_number() {
            json!(kind_number(&after.kind).ok_or("无法编码资源类型")?)
        } else {
            json!(after.kind)
        };
    }
    if changed(|e| &e.parent) {
        if after.parent.is_empty() {
            item.as_object_mut().unwrap().remove("parent");
        } else {
            item["parent"] = json!(after.parent);
        }
    }
    if before.is_none_or(|old| old.atlas != after.atlas) {
        item["atlas"] = json!(after.atlas);
    }
    if before.is_none_or(|old| old.region != after.region) {
        for field in ["ax", "ay", "aw", "ah"] {
            item.as_object_mut().unwrap().remove(field);
        }
        if let Some(r) = &after.region {
            for (field, number) in [("ax", r.x), ("ay", r.y), ("aw", r.width), ("ah", r.height)] {
                item[field] = json!(number);
            }
        }
    }
    Ok(())
}

fn context_matches(group: &Value, entry: &ResourceEntry) -> bool {
    text(group, "id").eq_ignore_ascii_case(&entry.subgroup)
        && (text(group, "res").is_empty() || text(group, "res") == entry.resolution)
        && text(group, "loc").eq_ignore_ascii_case(&entry.language)
}

fn item_matches(item: &Value, entry: &ResourceEntry) -> bool {
    let path = match &item["path"] {
        Value::Array(p) => p
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("/"),
        _ => text(item, "path"),
    };
    text(item, "id").eq_ignore_ascii_case(&entry.id)
        && (path.is_empty() || normalize_path(&path) == normalize_path(&entry.path))
        && (text(item, "parent").is_empty()
            || text(item, "parent").eq_ignore_ascii_case(&entry.parent))
        && entry.region.as_ref().is_none_or(|r| {
            [("ax", r.x), ("ay", r.y), ("aw", r.width), ("ah", r.height)]
                .iter()
                .all(|(k, n)| item[*k].is_null() || text(item, k).parse::<u32>().ok() == Some(*n))
        })
}

fn group_context(groups: &[Value], group: &Value) -> (String, String, String) {
    let id = text(group, "id");
    let inherited = groups.iter().find_map(|g| {
        g["subgroups"]
            .as_array()?
            .iter()
            .find(|s| text(s, "id").eq_ignore_ascii_case(&id))
            .map(|s| (text(g, "id"), text(s, "res")))
    });
    let parent = text(group, "parent");
    let res = text(group, "res");
    (
        if !parent.is_empty() {
            parent
        } else {
            inherited.as_ref().map(|p| p.0.clone()).unwrap_or(id)
        },
        if !res.is_empty() {
            res
        } else {
            inherited.map(|p| p.1).unwrap_or_default()
        },
        text(group, "loc"),
    )
}

fn destination(groups: &mut Vec<Value>, after: &ResourceEntry) -> Result<usize, String> {
    if let Some(index) = groups.iter().position(|g| {
        text(g, "id").eq_ignore_ascii_case(&after.subgroup) && g["resources"].is_array()
    }) {
        let group = &groups[index];
        let (parent, res, loc) = group_context(groups, group);
        if !parent.eq_ignore_ascii_case(&after.group)
            || res != after.resolution
            || !loc.eq_ignore_ascii_case(&after.language)
        {
            return Err(
                "目标子组的分组、分辨率或语言不同；请使用匹配的子组或输入新的子组名".into(),
            );
        }
        return Ok(index);
    }
    let mut group = json!({"type":"simple", "id":after.subgroup, "resources":[]});
    if !after.resolution.is_empty() {
        group["res"] = json!(
            after
                .resolution
                .parse::<u32>()
                .map_err(|_| "分辨率必须为非负整数或留空")?
        );
    }
    if !after.language.is_empty() {
        group["loc"] = json!(after.language);
    }
    if !after.group.eq_ignore_ascii_case(&after.subgroup) {
        group["parent"] = json!(after.group);
        let composite = match groups
            .iter()
            .position(|g| text(g, "id").eq_ignore_ascii_case(&after.group))
        {
            Some(i) => {
                if !groups[i]["subgroups"].is_array() {
                    return Err("目标资源组与已有普通子组重名".into());
                }
                i
            }
            None => {
                groups.push(json!({"type":"composite", "id":after.group, "subgroups":[]}));
                groups.len() - 1
            }
        };
        let mut reference = json!({"id":after.subgroup});
        if !after.resolution.is_empty() {
            reference["res"] = group["res"].clone();
        }
        groups[composite]["subgroups"]
            .as_array_mut()
            .unwrap()
            .push(reference);
    }
    groups.push(group);
    Ok(groups.len() - 1)
}

fn apply_groups(
    value: &mut Value,
    before: Option<&ResourceEntry>,
    after: Option<&ResourceEntry>,
    slot: u32,
) -> Result<usize, String> {
    let groups = value["groups"]
        .as_array_mut()
        .ok_or("清单 groups 不是数组")?;
    let mut matches = Vec::new();
    if let Some(before) = before {
        for (g, group) in groups
            .iter()
            .enumerate()
            .filter(|(_, g)| context_matches(g, before))
        {
            for (r, item) in group["resources"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                if item_matches(item, before) {
                    matches.push((g, r));
                }
            }
        }
        if matches.is_empty() {
            return Ok(0);
        }
        if matches.len() != 1 {
            return Err("清单中存在重复定义，不能确定要修改哪一条".into());
        }
    }
    let mut item = if let Some(&(g, r)) = matches.first() {
        groups[g]["resources"].as_array_mut().unwrap().remove(r)
    } else {
        json!({"slot":slot})
    };
    if let Some(after) = after {
        patch_item(&mut item, before, after)?;
        let target = destination(groups, after)?;
        if groups[target]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| text(r, "id").eq_ignore_ascii_case(&after.id))
        {
            return Err("目标子组已有同 ID 的资源".into());
        }
        // Keep the original order when the resource stays in the same group.
        let items = groups[target]["resources"].as_array_mut().unwrap();
        let index = matches
            .first()
            .filter(|(g, _)| *g == target)
            .map(|(_, r)| *r)
            .unwrap_or(items.len());
        items.insert(index.min(items.len()), item);
        if let Some(before) = before.filter(|before| before.id != after.id) {
            for group in groups.iter_mut().filter(|g| context_matches(g, before)) {
                for child in group["resources"].as_array_mut().into_iter().flatten() {
                    if text(child, "parent").eq_ignore_ascii_case(&before.id) {
                        child["parent"] = json!(after.id);
                    }
                }
            }
        }
        if before.is_none() {
            value["slot_count"] =
                json!(next_slot(value).max(slot.checked_add(1).ok_or("资源 slot 已耗尽")?));
        }
    }
    Ok(1)
}

fn apply_description(
    value: &mut Value,
    before: Option<&ResourceEntry>,
    after: Option<&ResourceEntry>,
) -> Result<usize, String> {
    let groups = value["groups"]
        .as_object_mut()
        .ok_or("资源描述缺少 groups")?;
    let mut found = None;
    if let Some(before) = before {
        for (g, group) in groups.iter() {
            for (s, subgroup) in group["subgroups"].as_object().into_iter().flatten() {
                if !s.eq_ignore_ascii_case(&before.subgroup)
                    || text(subgroup, "res") != before.resolution
                    || !text(subgroup, "language").eq_ignore_ascii_case(&before.language)
                {
                    continue;
                }
                for (id, item) in subgroup["resources"].as_object().into_iter().flatten() {
                    if id.eq_ignore_ascii_case(&before.id)
                        && normalize_path(&text(item, "path")) == normalize_path(&before.path)
                    {
                        if found.is_some() {
                            return Err("内嵌清单中有重复资源 ID".into());
                        }
                        found = Some((g.clone(), s.clone(), id.clone()));
                    }
                }
            }
        }
        if found.is_none() {
            return Ok(0);
        }
    }
    let mut item = if let Some((g, s, id)) = &found {
        groups[g]["subgroups"][s]["resources"]
            .as_object_mut()
            .unwrap()
            .remove(id)
            .unwrap()
    } else {
        json!({"properties":{}})
    };
    if let Some(after) = after {
        if before.is_none_or(|b| b.path != after.path) {
            item["path"] = json!(after.path);
        }
        if before.is_none_or(|b| b.kind != after.kind) {
            let kind: i32 = if after.kind == "Image" {
                0
            } else {
                after
                    .kind
                    .strip_prefix("Type ")
                    .and_then(|s| s.parse().ok())
                    .ok_or("内嵌资源描述的非图片类型需保留原始 Type 数字，不能推测类型编号")?
            };
            item["type"] = json!(kind);
        }
        if after.kind == "Image" && (after.atlas || !after.parent.is_empty()) {
            let mut ptx = item["ptx_info"].take();
            if !ptx.is_object() {
                ptx = json!({"imagetype":"0", "aflags":"0", "x":"0", "y":"0", "rows":"1", "cols":"1"});
            }
            ptx["parent"] = json!(after.parent);
            if before.is_none_or(|b| b.region != after.region) {
                let r = after.region.clone().unwrap_or_default();
                for (k, n) in [("ax", r.x), ("ay", r.y), ("aw", r.width), ("ah", r.height)] {
                    if n > u16::MAX.into() {
                        return Err("内嵌清单的图集坐标不能超过 65535".into());
                    }
                    ptx[k] = json!(n.to_string());
                }
            }
            item["ptx_info"] = ptx;
        } else {
            item.as_object_mut().unwrap().remove("ptx_info");
        }
        let group_key = groups
            .keys()
            .find(|g| {
                g.trim_end_matches("_CompositeShell")
                    .eq_ignore_ascii_case(&after.group)
            })
            .cloned()
            .unwrap_or_else(|| after.group.clone());
        let group = groups
            .entry(group_key)
            .or_insert_with(|| json!({"composite":after.group != after.subgroup,"subgroups":{}}));
        let subgroups = group["subgroups"]
            .as_object_mut()
            .ok_or("资源描述子组无效")?;
        let subgroup = subgroups.entry(after.subgroup.clone()).or_insert_with(
            || json!({"res":after.resolution,"language":after.language,"resources":{}}),
        );
        if text(subgroup, "res") != after.resolution || text(subgroup, "language") != after.language
        {
            return Err("目标子组的分辨率或语言不同".into());
        }
        let resources = subgroup["resources"]
            .as_object_mut()
            .ok_or("资源描述条目无效")?;
        if resources
            .keys()
            .any(|id| id.eq_ignore_ascii_case(&after.id))
        {
            return Err("目标子组已有同 ID 的资源".into());
        }
        resources.insert(after.id.clone(), item);
        if let Some(before) = before.filter(|b| b.id != after.id) {
            for group in groups.values_mut() {
                for (s, subgroup) in group["subgroups"].as_object_mut().into_iter().flatten() {
                    if s.eq_ignore_ascii_case(&before.subgroup)
                        && text(subgroup, "res") == before.resolution
                        && text(subgroup, "language").eq_ignore_ascii_case(&before.language)
                    {
                        for child in subgroup["resources"]
                            .as_object_mut()
                            .into_iter()
                            .flatten()
                            .map(|(_, v)| v)
                        {
                            if text(&child["ptx_info"], "parent").eq_ignore_ascii_case(&before.id) {
                                child["ptx_info"]["parent"] = json!(after.id);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(1)
}

fn new_rton(value: &Value) -> serde_rton::Value {
    use serde_rton::Value as R;
    match value {
        Value::Null => R::Null,
        Value::Bool(b) => R::Bool(*b),
        Value::String(s) => R::String(s.clone()),
        Value::Number(n) => {
            if let Some(n) = n.as_u64() {
                if let Ok(n) = u32::try_from(n) {
                    R::UInt32(n)
                } else {
                    R::UInt64(n)
                }
            } else if let Some(n) = n.as_i64() {
                if let Ok(n) = i32::try_from(n) {
                    R::Int32(n)
                } else {
                    R::Int64(n)
                }
            } else {
                R::Double(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::Array(items) => R::Array(items.iter().map(new_rton).collect()),
        Value::Object(items) => R::Object(
            items
                .iter()
                .map(|(k, v)| (k.clone(), new_rton(v)))
                .collect(),
        ),
    }
}

type ResourceValues<'a> = std::collections::HashMap<u64, Vec<(&'a serde_rton::Value, &'a Value)>>;

fn rton_field<'a>(value: &'a serde_rton::Value, key: &str) -> Option<&'a serde_rton::Value> {
    let serde_rton::Value::Object(items) = value else {
        return None;
    };
    items.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn indexed_resource<'a>(
    value: &Value,
    index: &ResourceValues<'a>,
) -> Option<(&'a serde_rton::Value, &'a Value)> {
    let id = value["id"].as_str()?;
    let candidates = index.get(&value["slot"].as_u64()?)?;
    if candidates.len() == 1 {
        return Some(candidates[0]);
    }
    let mut same_id = candidates
        .iter()
        .filter(|(_, v)| v["id"].as_str() == Some(id));
    let first = *same_id.next()?;
    same_id.next().is_none().then_some(first)
}

fn reconcile_rton(original: &serde_rton::Value, old: &Value, new: &Value) -> serde_rton::Value {
    let mut index = ResourceValues::new();
    if let Some(serde_rton::Value::Array(groups)) = rton_field(original, "groups") {
        for (group, json) in groups
            .iter()
            .zip(old["groups"].as_array().into_iter().flatten())
        {
            if let Some(serde_rton::Value::Array(items)) = rton_field(group, "resources") {
                for (item, json) in items
                    .iter()
                    .zip(json["resources"].as_array().into_iter().flatten())
                {
                    if let Some(slot) = json["slot"].as_u64() {
                        index.entry(slot).or_default().push((item, json));
                    }
                }
            }
        }
    }
    reconcile_values(original, old, new, &index)
}

// Hydrate newly inserted groups from original resource records too, so moving a
// resource between groups does not erase opaque RTON types inside its fields.
fn hydrate_rton(value: &Value, index: &ResourceValues<'_>) -> serde_rton::Value {
    use serde_rton::Value as R;
    if let Some((original, old)) = indexed_resource(value, index) {
        return reconcile_values(original, old, value, index);
    }
    match value {
        Value::Object(items) => R::Object(
            items
                .iter()
                .map(|(k, v)| (k.clone(), hydrate_rton(v, index)))
                .collect(),
        ),
        Value::Array(items) => R::Array(items.iter().map(|v| hydrate_rton(v, index)).collect()),
        _ => new_rton(value),
    }
}

fn reconcile_values(
    original: &serde_rton::Value,
    old: &Value,
    new: &Value,
    index: &ResourceValues<'_>,
) -> serde_rton::Value {
    use serde_rton::Value as R;
    if old == new {
        return original.clone();
    }
    match (original, old, new) {
        (R::Object(items), Value::Object(before), Value::Object(after)) => {
            let mut output = Vec::new();
            for (key, value) in items {
                if let Some(next) = after.get(key) {
                    output.push((
                        key.clone(),
                        reconcile_values(value, &before[key], next, index),
                    ));
                }
            }
            for (key, value) in after {
                if !before.contains_key(key) {
                    output.push((key.clone(), hydrate_rton(value, index)));
                }
            }
            R::Object(output)
        }
        (R::Array(items), Value::Array(before), Value::Array(after)) => {
            let by_id = before
                .iter()
                .enumerate()
                .filter_map(|(i, v)| Some((v["id"].as_str()?, i)))
                .collect::<std::collections::HashMap<_, _>>();
            R::Array(
                after
                    .iter()
                    .enumerate()
                    .map(|(i, next)| {
                        if let Some((original, old)) = indexed_resource(next, index) {
                            return reconcile_values(original, old, next, index);
                        }
                        let found = next
                            .get("id")
                            .and_then(Value::as_str)
                            .and_then(|id| by_id.get(id).copied())
                            .or_else(|| (i < items.len()).then_some(i));
                        match found {
                            Some(i) => reconcile_values(&items[i], &before[i], next, index),
                            None => hydrate_rton(next, index),
                        }
                    })
                    .collect(),
            )
        }
        _ => hydrate_rton(new, index),
    }
}
