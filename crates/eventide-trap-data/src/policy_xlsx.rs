//! Trap policy Excel (xlsx) import / export.

use anyhow::{Context, Result};
use calamine::{Data, Reader, Xlsx};
use rust_xlsxwriter::{Format, Workbook, Worksheet};
use std::collections::BTreeMap;
use std::io::Cursor;

use crate::mib_store::TrapPolicyDraft;
use crate::policy_store::TrapPolicy;

const HEADERS: &[&str] = &[
    "名称",
    "TrapOID",
    "匹配",
    "级别",
    "启用",
    "摘要模板",
    "描述",
    "OBJECTS",
    "OID映射",
    "关键字",
    "模块",
    "恢复OID",
    "恢复值",
    "告警标识OID",
    "级别OID",
    "级别映射",
];

/// Write policies (or drafts) to an .xlsx workbook bytes.
pub fn write_xlsx_from_drafts(drafts: &[TrapPolicyDraft]) -> Result<Vec<u8>> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Trap策略")?;

    let header_fmt = Format::new().set_bold();
    for (col, h) in HEADERS.iter().enumerate() {
        sheet.write_string_with_format(0, col as u16, *h, &header_fmt)?;
    }

    for (row_i, d) in drafts.iter().enumerate() {
        let r = (row_i + 1) as u32;
        write_row(sheet, r, d)?;
    }

    let widths: &[f64] = &[
        18.0, 36.0, 8.0, 10.0, 8.0, 40.0, 28.0, 28.0, 36.0, 16.0, 18.0, 28.0, 16.0, 28.0, 28.0,
        28.0,
    ];
    for (i, w) in widths.iter().enumerate() {
        sheet.set_column_width(i as u16, *w)?;
    }

    Ok(workbook.save_to_buffer()?)
}

pub fn write_xlsx_from_policies(policies: &[TrapPolicy]) -> Result<Vec<u8>> {
    let drafts: Vec<TrapPolicyDraft> = policies
        .iter()
        .map(|p| TrapPolicyDraft {
            name: p.name.clone(),
            trap_oid: p.trap_oid.clone(),
            match_mode: p.match_mode.clone(),
            severity: p.severity.clone(),
            enabled: p.enabled,
            summary_template: p.summary_template.clone(),
            description: p.description.clone(),
            objects: p.objects.clone(),
            object_oids: p.object_oids.clone(),
            keywords: p.keywords.clone(),
            module: p.module.clone(),
            status: p.status.clone(),
            resolve_oid: p.resolve_oid.clone(),
            resolve_values: p.resolve_values.clone(),
            fingerprint_oids: p.fingerprint_oids.clone(),
            severity_oid: p.severity_oid.clone(),
            severity_map: p.severity_map.clone(),
        })
        .collect();
    write_xlsx_from_drafts(&drafts)
}

fn write_row(sheet: &mut Worksheet, row: u32, d: &TrapPolicyDraft) -> Result<()> {
    sheet.write_string(row, 0, &d.name)?;
    sheet.write_string(row, 1, &d.trap_oid)?;
    sheet.write_string(row, 2, &d.match_mode)?;
    sheet.write_string(row, 3, &d.severity)?;
    sheet.write_string(row, 4, if d.enabled { "是" } else { "否" })?;
    sheet.write_string(row, 5, &d.summary_template)?;
    sheet.write_string(row, 6, &d.description)?;
    sheet.write_string(row, 7, &d.objects.join(", "))?;
    sheet.write_string(row, 8, &encode_kv_map(&d.object_oids))?;
    sheet.write_string(row, 9, &d.keywords.join(", "))?;
    sheet.write_string(row, 10, &d.module)?;
    let resolve_oid = if d.resolve_oid.is_empty() && d.status == "resolved" {
        ""
    } else {
        d.resolve_oid.as_str()
    };
    let resolve_vals = if d.resolve_values.is_empty() && d.status == "resolved" {
        "resolved".to_string()
    } else {
        d.resolve_values.join(", ")
    };
    sheet.write_string(row, 11, resolve_oid)?;
    sheet.write_string(row, 12, &resolve_vals)?;
    sheet.write_string(row, 13, &d.fingerprint_oids.join(", "))?;
    sheet.write_string(row, 14, &d.severity_oid)?;
    sheet.write_string(row, 15, &encode_kv_map(&d.severity_map))?;
    Ok(())
}

fn encode_kv_map(map: &BTreeMap<String, String>) -> String {
    map.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(";")
}

fn decode_kv_map(s: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for part in s.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((k, v)) = part.split_once('=') {
            let k = k.trim();
            let v = v.trim();
            if !k.is_empty() && !v.is_empty() {
                out.insert(k.to_string(), v.to_string());
            }
        }
    }
    out
}

fn split_list(s: &str) -> Vec<String> {
    s.split([',', '，', ';', '；'])
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

fn cell_str(v: &Data) -> String {
    match v {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if f.fract() == 0.0 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => {
            if *b {
                "是".into()
            } else {
                "否".into()
            }
        }
        Data::DateTime(dt) => format!("{dt:?}"),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("ERR:{e:?}"),
    }
}

fn parse_enabled(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "是" | "启用" | "on"
    )
}

fn normalize_header(h: &str) -> String {
    h.trim()
        .to_ascii_lowercase()
        .replace([' ', '_', '-'], "")
}

fn map_header(h: &str) -> Option<&'static str> {
    let n = normalize_header(h);
    match n.as_str() {
        "名称" | "name" | "alertname" | "策略名" => Some("name"),
        "trapoid" | "oid" | "trap_oid" => Some("trap_oid"),
        "匹配" | "匹配方式" | "match" | "matchmode" => Some("match_mode"),
        "级别" | "severity" | "level" => Some("severity"),
        "启用" | "enabled" | "enable" => Some("enabled"),
        "摘要模板" | "摘要" | "summary" | "summarytemplate" | "template" => {
            Some("summary_template")
        }
        "描述" | "description" | "desc" => Some("description"),
        "objects" | "object" | "varbinds" => Some("objects"),
        "oid映射" | "objectoids" | "object_oids" | "oidmap" => Some("object_oids"),
        "关键字" | "keywords" | "keyword" => Some("keywords"),
        "模块" | "module" | "mib" => Some("module"),
        "状态" | "status" => Some("status"),
        "恢复oid" | "resolveoid" | "resolve_oid" | "恢复判定oid" => Some("resolve_oid"),
        "恢复值" | "resolvevalues" | "resolve_values" | "恢复判定值" => Some("resolve_values"),
        "指纹oid" | "fingerprintoid" | "fingerprint_oids" | "指纹" | "流水号oid"
        | "告警标识oid" | "告警标识" | "标识oid" => {
            Some("fingerprint_oids")
        }
        "级别oid" | "severityoid" | "severity_oid" => Some("severity_oid"),
        "级别映射" | "severitymap" | "severity_map" => Some("severity_map"),
        _ => None,
    }
}

/// Parse first sheet of an .xlsx into policy drafts.
pub fn read_xlsx_drafts(bytes: &[u8]) -> Result<Vec<TrapPolicyDraft>> {
    let cursor = Cursor::new(bytes);
    let mut workbook: Xlsx<_> =
        Xlsx::new(cursor).context("打开 xlsx 失败（请确认是 Excel .xlsx 文件）")?;
    let sheet_names = workbook.sheet_names().to_vec();
    let sheet_name = sheet_names
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("xlsx 中没有工作表"))?;
    let range = workbook
        .worksheet_range(&sheet_name)
        .with_context(|| format!("读取工作表 `{sheet_name}`"))?;

    let mut rows = range.rows();
    let header_row = rows.next().ok_or_else(|| anyhow::anyhow!("xlsx 为空"))?;
    let mut col_map: BTreeMap<usize, &'static str> = BTreeMap::new();
    for (i, cell) in header_row.iter().enumerate() {
        let h = cell_str(cell);
        if let Some(key) = map_header(&h) {
            col_map.insert(i, key);
        }
    }
    if !col_map.values().any(|k| *k == "trap_oid") {
        anyhow::bail!("缺少列「TrapOID」；请使用系统导出的 Excel 模板");
    }

    let mut out = Vec::new();
    for row in rows {
        let mut fields: BTreeMap<&str, String> = BTreeMap::new();
        for (i, cell) in row.iter().enumerate() {
            if let Some(key) = col_map.get(&i) {
                fields.insert(*key, cell_str(cell));
            }
        }
        let trap_oid = fields.get("trap_oid").cloned().unwrap_or_default();
        if trap_oid.trim().is_empty() {
            continue;
        }
        let name = fields
            .get("name")
            .cloned()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| trap_oid.clone());
        let match_mode = fields
            .get("match_mode")
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| s == "prefix" || s == "前缀")
            .map(|_| "prefix".to_string())
            .unwrap_or_else(|| "exact".into());
        let severity = fields
            .get("severity")
            .cloned()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "warning".into());
        let enabled = fields
            .get("enabled")
            .map(|s| parse_enabled(s))
            .unwrap_or(true);
        let objects = fields
            .get("objects")
            .map(|s| split_list(s))
            .unwrap_or_default();
        let object_oids = fields
            .get("object_oids")
            .map(|s| decode_kv_map(s))
            .unwrap_or_default();
        let keywords = fields
            .get("keywords")
            .map(|s| split_list(s))
            .unwrap_or_default();
        let resolve_oid = fields
            .get("resolve_oid")
            .cloned()
            .unwrap_or_default()
            .trim()
            .to_string();
        let mut resolve_values = fields
            .get("resolve_values")
            .map(|s| split_list(s))
            .unwrap_or_default();
        let legacy_status = fields.get("status").cloned().unwrap_or_default();
        let status = if resolve_oid.is_empty()
            && (resolve_values.is_empty()
                || (resolve_values.len() == 1
                    && crate::policy_store::normalize_event_status(&resolve_values[0])
                        == "resolved"))
        {
            if resolve_values.len() == 1
                && crate::policy_store::normalize_event_status(&resolve_values[0]) == "resolved"
            {
                resolve_values.clear();
                "resolved".into()
            } else if crate::policy_store::normalize_event_status(&legacy_status) == "resolved" {
                "resolved".into()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        out.push(TrapPolicyDraft {
            name,
            trap_oid: trap_oid.trim().to_string(),
            match_mode,
            severity,
            enabled,
            summary_template: fields.get("summary_template").cloned().unwrap_or_default(),
            description: fields.get("description").cloned().unwrap_or_default(),
            objects,
            object_oids,
            keywords,
            module: fields.get("module").cloned().unwrap_or_default(),
            status,
            resolve_oid,
            resolve_values,
            fingerprint_oids: fields
                .get("fingerprint_oids")
                .map(|s| split_list(s))
                .unwrap_or_default(),
            severity_oid: fields
                .get("severity_oid")
                .cloned()
                .unwrap_or_default()
                .trim()
                .to_string(),
            severity_map: fields
                .get("severity_map")
                .map(|s| decode_kv_map(s))
                .unwrap_or_default(),
        });
    }
    if out.is_empty() {
        anyhow::bail!("未读到任何策略行（请检查 TrapOID 列）");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_xlsx() {
        let drafts = vec![TrapPolicyDraft {
            name: "linkDown".into(),
            trap_oid: "1.3.6.1.6.3.1.1.5.3".into(),
            match_mode: "exact".into(),
            severity: "disaster".into(),
            enabled: true,
            summary_template: "${alertname} on ${ip}".into(),
            description: "iface down".into(),
            objects: vec!["ifIndex".into()],
            object_oids: BTreeMap::from([("ifIndex".into(), "1.3.6.1.2.1.2.2.1.1".into())]),
            keywords: vec!["link".into()],
            module: "IF-MIB".into(),
            status: "".into(),
            resolve_oid: "1.3.6.1.2.1.2.2.1.8".into(),
            resolve_values: vec!["1".into(), "up".into()],
            fingerprint_oids: vec!["1.3.6.1.2.1.2.2.1.1".into()],
            severity_oid: "1.3.6.1.4.1.2011.2.91.10.3.1.1.6".into(),
            severity_map: BTreeMap::from([
                ("1".into(), "disaster".into()),
                ("2".into(), "high".into()),
            ]),
        }];
        let bytes = write_xlsx_from_drafts(&drafts).unwrap();
        let back = read_xlsx_drafts(&bytes).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].name, "linkDown");
        assert_eq!(back[0].resolve_oid, "1.3.6.1.2.1.2.2.1.8");
        assert_eq!(
            back[0].fingerprint_oids,
            vec!["1.3.6.1.2.1.2.2.1.1".to_string()]
        );
        assert_eq!(
            back[0].severity_map.get("1").map(|s| s.as_str()),
            Some("disaster")
        );
    }
}
