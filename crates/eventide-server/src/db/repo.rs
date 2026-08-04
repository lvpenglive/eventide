//! Repository helpers mapping MySQL rows to core models.

use super::Db;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use eventide_core::*;
use mysql::prelude::*;
use mysql::{Params, Row, Value};
use std::collections::BTreeMap;
use uuid::Uuid;

fn parse_dt(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .or_else(|_| {
            s.parse::<DateTime<Utc>>()
                .map_err(|e| anyhow!("bad datetime {s}: {e}"))
        })
}

fn fmt_dt(t: DateTime<Utc>) -> String {
    t.to_rfc3339()
}

fn positional(vals: Vec<Value>) -> Params {
    Params::Positional(vals)
}

fn labels_from_json(s: &str) -> Result<Labels> {
    Ok(serde_json::from_str(s).unwrap_or_default())
}

fn uuid_ids_from_json(s: &str) -> Result<Vec<Uuid>> {
    let raw: Vec<String> = serde_json::from_str(s).unwrap_or_default();
    raw.into_iter()
        .map(|x| Uuid::parse_str(&x).map_err(|e| anyhow!(e)))
        .collect()
}

fn col_str(row: &Row, idx: usize) -> Result<String> {
    match row.get_opt::<String, _>(idx) {
        Some(Ok(s)) => Ok(s),
        Some(Err(e)) => Err(anyhow!("bad/null string col {idx}: {e}")),
        None => Err(anyhow!("missing string col {idx}")),
    }
}

fn col_str_opt(row: &Row, idx: usize) -> Option<String> {
    match row.get_opt::<String, _>(idx) {
        Some(Ok(s)) => Some(s),
        _ => None,
    }
}

fn col_i64(row: &Row, idx: usize) -> i64 {
    match row.get_opt::<i64, _>(idx) {
        Some(Ok(v)) => v,
        _ => 0,
    }
}

fn col_f64_opt(row: &Row, idx: usize) -> Option<f64> {
    match row.get_opt::<f64, _>(idx) {
        Some(Ok(v)) => Some(v),
        _ => None,
    }
}

fn col_uuid(row: &Row, idx: usize) -> Result<Uuid> {
    Uuid::parse_str(&col_str(row, idx)?).map_err(|e| anyhow!(e))
}

fn v(x: impl Into<Value>) -> Value {
    x.into()
}

impl Db {
    // ---------- datasources ----------

    pub fn list_datasources(&self) -> Result<Vec<Datasource>> {
        let mut conn = self.conn()?;
        let rows: Vec<Row> = conn.query(
            "SELECT id, name, kind, url, enabled, created_at, updated_at,
                    COALESCE(options_json, '{}') FROM datasources ORDER BY name",
        )?;
        rows.iter().map(map_datasource).collect()
    }

    pub fn get_datasource(&self, id: Uuid) -> Result<Option<Datasource>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT id, name, kind, url, enabled, created_at, updated_at,
                    COALESCE(options_json, '{}') FROM datasources WHERE id=?",
            positional(vec![v(id.to_string())]),
        )?;
        Ok(row.as_ref().map(map_datasource).transpose()?)
    }

    pub fn upsert_datasource(&self, ds: &Datasource) -> Result<()> {
        let mut conn = self.conn()?;
        conn.exec_drop(
            "INSERT INTO datasources (id, name, kind, url, enabled, created_at, updated_at, options_json)
             VALUES (?,?,?,?,?,?,?,?)
             ON DUPLICATE KEY UPDATE
               name=VALUES(name), kind=VALUES(kind), url=VALUES(url),
               enabled=VALUES(enabled), updated_at=VALUES(updated_at),
               options_json=VALUES(options_json)",
            positional(vec![
                v(ds.id.to_string()),
                v(ds.name.as_str()),
                v(ds.kind.as_str()),
                v(ds.url.as_str()),
                v(ds.enabled as i64),
                v(fmt_dt(ds.created_at)),
                v(fmt_dt(ds.updated_at)),
                v(serde_json::to_string(&ds.options)?),
            ]),
        )?;
        Ok(())
    }

    pub fn delete_datasource(&self, id: Uuid) -> Result<bool> {
        let mut conn = self.conn()?;
        let n = conn
            .exec_iter(
                "DELETE FROM datasources WHERE id=?",
                positional(vec![v(id.to_string())]),
            )?
            .affected_rows();
        Ok(n > 0)
    }

    // ---------- channels ----------

    pub fn list_channels(&self) -> Result<Vec<NotifyChannel>> {
        let mut conn = self.conn()?;
        let rows: Vec<Row> = conn.query(
            "SELECT id, name, kind, url, secret, enabled, created_at, updated_at,
                    COALESCE(options_json, '{}')
             FROM notify_channels ORDER BY name",
        )?;
        rows.iter().map(map_channel).collect()
    }

    pub fn get_channel(&self, id: Uuid) -> Result<Option<NotifyChannel>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT id, name, kind, url, secret, enabled, created_at, updated_at,
                    COALESCE(options_json, '{}')
             FROM notify_channels WHERE id=?",
            positional(vec![v(id.to_string())]),
        )?;
        Ok(row.as_ref().map(map_channel).transpose()?)
    }

    pub fn upsert_channel(&self, ch: &NotifyChannel) -> Result<()> {
        let mut conn = self.conn()?;
        conn.exec_drop(
            "INSERT INTO notify_channels (id, name, kind, url, secret, enabled, created_at, updated_at, options_json)
             VALUES (?,?,?,?,?,?,?,?,?)
             ON DUPLICATE KEY UPDATE
               name=VALUES(name), kind=VALUES(kind), url=VALUES(url), secret=VALUES(secret),
               enabled=VALUES(enabled), updated_at=VALUES(updated_at),
               options_json=VALUES(options_json)",
            positional(vec![
                v(ch.id.to_string()),
                v(ch.name.as_str()),
                v(ch.kind.as_str()),
                v(ch.url.as_str()),
                v(ch.secret.clone()),
                v(ch.enabled as i64),
                v(fmt_dt(ch.created_at)),
                v(fmt_dt(ch.updated_at)),
                v(serde_json::to_string(&ch.options)?),
            ]),
        )?;
        Ok(())
    }

    pub fn delete_channel(&self, id: Uuid) -> Result<bool> {
        let mut conn = self.conn()?;
        let n = conn
            .exec_iter(
                "DELETE FROM notify_channels WHERE id=?",
                positional(vec![v(id.to_string())]),
            )?
            .affected_rows();
        Ok(n > 0)
    }

    // ---------- rules ----------

    pub fn list_rules(&self) -> Result<Vec<Rule>> {
        let mut conn = self.conn()?;
        let rows: Vec<Row> = conn.query(
            "SELECT id, name, datasource_id, expr, comparator, threshold, for_seconds, interval_seconds,
                    severity, labels_json, annotations_json, channel_ids_json, enabled, created_at, updated_at
             FROM rules ORDER BY name",
        )?;
        rows.iter().map(map_rule).collect()
    }

    pub fn list_enabled_rules_due(&self, now: DateTime<Utc>) -> Result<Vec<Rule>> {
        let rules = self.list_rules()?;
        let mut conn = self.conn()?;
        let mut due = Vec::new();
        for rule in rules {
            if !rule.enabled {
                continue;
            }
            let last_row: Option<Row> = conn.exec_first(
                "SELECT last_run_at FROM rules WHERE id=?",
                positional(vec![v(rule.id.to_string())]),
            )?;
            let last = last_row.as_ref().and_then(|r| col_str_opt(r, 0));
            let should = match last {
                None => true,
                Some(s) => {
                    let t = parse_dt(&s)?;
                    (now - t).num_seconds() >= rule.interval_seconds as i64
                }
            };
            if should {
                due.push(rule);
            }
        }
        Ok(due)
    }

    pub fn touch_rule_run(&self, id: Uuid, at: DateTime<Utc>) -> Result<()> {
        let mut conn = self.conn()?;
        conn.exec_drop(
            "UPDATE rules SET last_run_at=? WHERE id=?",
            positional(vec![v(fmt_dt(at)), v(id.to_string())]),
        )?;
        Ok(())
    }

    pub fn get_rule(&self, id: Uuid) -> Result<Option<Rule>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT id, name, datasource_id, expr, comparator, threshold, for_seconds, interval_seconds,
                    severity, labels_json, annotations_json, channel_ids_json, enabled, created_at, updated_at
             FROM rules WHERE id=?",
            positional(vec![v(id.to_string())]),
        )?;
        Ok(row.as_ref().map(map_rule).transpose()?)
    }

    pub fn upsert_rule(&self, rule: &Rule) -> Result<()> {
        let mut conn = self.conn()?;
        let channel_ids: Vec<String> = rule.channel_ids.iter().map(|u| u.to_string()).collect();
        conn.exec_drop(
            "INSERT INTO rules (
                id, name, datasource_id, expr, comparator, threshold, for_seconds, interval_seconds,
                severity, labels_json, annotations_json, channel_ids_json, enabled, created_at, updated_at
             ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
             ON DUPLICATE KEY UPDATE
               name=VALUES(name), datasource_id=VALUES(datasource_id), expr=VALUES(expr),
               comparator=VALUES(comparator), threshold=VALUES(threshold),
               for_seconds=VALUES(for_seconds), interval_seconds=VALUES(interval_seconds),
               severity=VALUES(severity), labels_json=VALUES(labels_json),
               annotations_json=VALUES(annotations_json), channel_ids_json=VALUES(channel_ids_json),
               enabled=VALUES(enabled), updated_at=VALUES(updated_at)",
            positional(vec![
                v(rule.id.to_string()),
                v(rule.name.as_str()),
                v(rule.datasource_id.to_string()),
                v(rule.expr.as_str()),
                v(rule.comparator.as_str()),
                v(rule.threshold),
                v(rule.for_seconds as i64),
                v(rule.interval_seconds as i64),
                v(rule.severity.as_str()),
                v(serde_json::to_string(&rule.labels)?),
                v(serde_json::to_string(&rule.annotations)?),
                v(serde_json::to_string(&channel_ids)?),
                v(rule.enabled as i64),
                v(fmt_dt(rule.created_at)),
                v(fmt_dt(rule.updated_at)),
            ]),
        )?;
        Ok(())
    }

    pub fn delete_rule(&self, id: Uuid) -> Result<bool> {
        let mut conn = self.conn()?;
        let n = conn
            .exec_iter(
                "DELETE FROM rules WHERE id=?",
                positional(vec![v(id.to_string())]),
            )?
            .affected_rows();
        Ok(n > 0)
    }

    // ---------- alert events ----------

    pub fn list_alerts(&self, status: Option<&str>) -> Result<Vec<AlertEvent>> {
        let mut conn = self.conn()?;
        if let Some(st) = status {
            let rows: Vec<Row> = conn.exec(
                "SELECT id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                        value, starts_at, ends_at, pending_since, last_evaluated_at,
                        notified_firing, notified_resolved
                 FROM alert_events WHERE status=? ORDER BY last_evaluated_at DESC LIMIT 1000",
                positional(vec![v(st)]),
            )?;
            rows.iter().map(map_alert).collect()
        } else {
            let rows: Vec<Row> = conn.query(
                "SELECT id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                        value, starts_at, ends_at, pending_since, last_evaluated_at,
                        notified_firing, notified_resolved
                 FROM alert_events ORDER BY last_evaluated_at DESC LIMIT 1000",
            )?;
            rows.iter().map(map_alert).collect()
        }
    }

    pub fn alert_status_counts(&self) -> Result<(usize, usize, usize, usize)> {
        let mut conn = self.conn()?;
        let mut firing = 0usize;
        let mut pending = 0usize;
        let mut resolved = 0usize;
        let rows: Vec<Row> =
            conn.query("SELECT status, COUNT(*) FROM alert_events GROUP BY status")?;
        for row in rows {
            let status = col_str(&row, 0)?;
            let n = col_i64(&row, 1) as usize;
            match status.as_str() {
                "firing" => firing = n,
                "pending" => pending = n,
                "resolved" => resolved = n,
                _ => {}
            }
        }
        Ok((firing, pending, resolved, firing + pending + resolved))
    }

    pub fn list_recent_alerts_overview(&self, limit: usize) -> Result<Vec<AlertEvent>> {
        let mut conn = self.conn()?;
        let lim = limit.max(1) as i64;
        let rows: Vec<Row> = conn.exec(
            "SELECT id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                    value, starts_at, ends_at, pending_since, last_evaluated_at,
                    notified_firing, notified_resolved
             FROM alert_events
             ORDER BY CASE status
                 WHEN 'firing' THEN 0
                 WHEN 'pending' THEN 1
                 ELSE 2
             END,
             last_evaluated_at DESC
             LIMIT ?",
            positional(vec![v(lim)]),
        )?;
        rows.iter().map(map_alert).collect()
    }

    pub fn count_table(&self, table: &str) -> Result<usize> {
        let allowed = [
            "datasources",
            "rules",
            "notify_channels",
            "ingress_routes",
            "lookup_tables",
            "enrich_rules",
        ];
        if !allowed.contains(&table) {
            anyhow::bail!("count_table: unsupported table");
        }
        let mut conn = self.conn()?;
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let n: i64 = conn.query_first(sql)?.unwrap_or(0);
        Ok(n as usize)
    }

    pub fn count_enabled_rules(&self) -> Result<usize> {
        let mut conn = self.conn()?;
        let n: i64 = conn
            .query_first("SELECT COUNT(*) FROM rules WHERE enabled=1")?
            .unwrap_or(0);
        Ok(n as usize)
    }

    pub fn list_recent_notify_skips(&self, limit: usize) -> Result<Vec<(String, String, String)>> {
        let mut conn = self.conn()?;
        let lim = limit.max(1) as i64;
        let rows: Vec<Row> = conn.exec(
            "SELECT COALESCE(error, ''), transition, created_at
             FROM notify_logs
             WHERE success=0 AND error IS NOT NULL AND error != ''
             ORDER BY created_at DESC
             LIMIT ?",
            positional(vec![v(lim)]),
        )?;
        let mut out = Vec::new();
        for row in rows {
            out.push((
                col_str(&row, 0).unwrap_or_default(),
                col_str(&row, 1).unwrap_or_default(),
                col_str(&row, 2).unwrap_or_default(),
            ));
        }
        Ok(out)
    }

    pub fn get_alert_by_fingerprint(&self, fp: &str) -> Result<Option<AlertEvent>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                    value, starts_at, ends_at, pending_since, last_evaluated_at,
                    notified_firing, notified_resolved
             FROM alert_events WHERE fingerprint=?",
            positional(vec![v(fp)]),
        )?;
        Ok(row.as_ref().map(map_alert).transpose()?)
    }

    pub fn get_alert(&self, id: Uuid) -> Result<Option<AlertEvent>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                    value, starts_at, ends_at, pending_since, last_evaluated_at,
                    notified_firing, notified_resolved
             FROM alert_events WHERE id=?",
            positional(vec![v(id.to_string())]),
        )?;
        Ok(row.as_ref().map(map_alert).transpose()?)
    }

    pub fn list_notify_logs_for_alert(&self, alert_id: Uuid) -> Result<Vec<NotifyLog>> {
        let mut conn = self.conn()?;
        let rows: Vec<Row> = conn.exec(
            "SELECT id, alert_id, channel_id, transition, success, error, created_at,
                    COALESCE(body, '')
             FROM notify_logs WHERE alert_id=? ORDER BY created_at DESC LIMIT 100",
            positional(vec![v(alert_id.to_string())]),
        )?;
        rows.iter().map(map_notify_log).collect()
    }

    pub fn list_notify_logs(
        &self,
        channel_id: Option<Uuid>,
        success: Option<bool>,
        q: Option<&str>,
        limit: usize,
    ) -> Result<Vec<NotifyLog>> {
        let mut conn = self.conn()?;
        let lim = limit.clamp(1, 500) as i64;
        let mut sql = String::from(
            "SELECT id, alert_id, channel_id, transition, success, error, created_at,
                    COALESCE(body, '')
             FROM notify_logs WHERE 1=1",
        );
        let mut vals: Vec<Value> = Vec::new();
        if let Some(cid) = channel_id {
            sql.push_str(" AND channel_id=?");
            vals.push(v(cid.to_string()));
        }
        if let Some(ok) = success {
            sql.push_str(" AND success=?");
            vals.push(v(if ok { 1i64 } else { 0i64 }));
        }
        if let Some(q) = q.map(str::trim).filter(|s| !s.is_empty()) {
            sql.push_str(" AND (body LIKE ? OR IFNULL(error,'') LIKE ? OR alert_id LIKE ?)");
            let pat = format!("%{q}%");
            vals.push(v(pat.clone()));
            vals.push(v(pat.clone()));
            vals.push(v(pat));
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT ?");
        vals.push(v(lim));
        let rows: Vec<Row> = conn.exec(sql, positional(vals))?;
        rows.iter().map(map_notify_log).collect()
    }

    pub fn alerts_for_rule(&self, rule_id: Uuid) -> Result<BTreeMap<String, AlertEvent>> {
        let mut conn = self.conn()?;
        let rows: Vec<Row> = conn.exec(
            "SELECT id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                    value, starts_at, ends_at, pending_since, last_evaluated_at,
                    notified_firing, notified_resolved
             FROM alert_events WHERE rule_id=?",
            positional(vec![v(rule_id.to_string())]),
        )?;
        let mut map = BTreeMap::new();
        for row in rows {
            let ev = map_alert(&row)?;
            map.insert(ev.fingerprint.clone(), ev);
        }
        Ok(map)
    }

    pub fn upsert_alert(&self, ev: &AlertEvent) -> Result<()> {
        let mut conn = self.conn()?;
        conn.exec_drop(
            "INSERT INTO alert_events (
                id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                value, starts_at, ends_at, pending_since, last_evaluated_at,
                notified_firing, notified_resolved
             ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)
             ON DUPLICATE KEY UPDATE
               id=VALUES(id), status=VALUES(status), severity=VALUES(severity),
               labels_json=VALUES(labels_json), annotations_json=VALUES(annotations_json),
               value=VALUES(value), starts_at=VALUES(starts_at), ends_at=VALUES(ends_at),
               pending_since=VALUES(pending_since), last_evaluated_at=VALUES(last_evaluated_at),
               notified_firing=VALUES(notified_firing), notified_resolved=VALUES(notified_resolved)",
            positional(vec![
                v(ev.id.to_string()),
                v(ev.rule_id.to_string()),
                v(ev.fingerprint.as_str()),
                v(ev.status.as_str()),
                v(ev.severity.as_str()),
                v(serde_json::to_string(&ev.labels)?),
                v(serde_json::to_string(&ev.annotations)?),
                v(ev.value),
                v(fmt_dt(ev.starts_at)),
                v(ev.ends_at.map(fmt_dt)),
                v(ev.pending_since.map(fmt_dt)),
                v(fmt_dt(ev.last_evaluated_at)),
                v(ev.notified_firing as i64),
                v(ev.notified_resolved as i64),
            ]),
        )?;
        Ok(())
    }

    // ---------- silences ----------

    pub fn list_silences(&self) -> Result<Vec<Silence>> {
        let mut conn = self.conn()?;
        let rows: Vec<Row> = conn.query(
            "SELECT id, rule_id, matchers_json, starts_at, ends_at, comment, created_at FROM silences ORDER BY created_at DESC",
        )?;
        rows.iter().map(map_silence).collect()
    }

    pub fn active_silences(&self, now: DateTime<Utc>) -> Result<Vec<Silence>> {
        Ok(self
            .list_silences()?
            .into_iter()
            .filter(|s| now >= s.starts_at && now < s.ends_at)
            .collect())
    }

    pub fn upsert_silence(&self, s: &Silence) -> Result<()> {
        let mut conn = self.conn()?;
        conn.exec_drop(
            "INSERT INTO silences (id, rule_id, matchers_json, starts_at, ends_at, comment, created_at)
             VALUES (?,?,?,?,?,?,?)
             ON DUPLICATE KEY UPDATE
               rule_id=VALUES(rule_id), matchers_json=VALUES(matchers_json),
               starts_at=VALUES(starts_at), ends_at=VALUES(ends_at), comment=VALUES(comment)",
            positional(vec![
                v(s.id.to_string()),
                v(s.rule_id.map(|u| u.to_string())),
                v(serde_json::to_string(&s.matchers)?),
                v(fmt_dt(s.starts_at)),
                v(fmt_dt(s.ends_at)),
                v(s.comment.as_str()),
                v(fmt_dt(s.created_at)),
            ]),
        )?;
        Ok(())
    }

    pub fn delete_silence(&self, id: Uuid) -> Result<bool> {
        let mut conn = self.conn()?;
        let n = conn
            .exec_iter(
                "DELETE FROM silences WHERE id=?",
                positional(vec![v(id.to_string())]),
            )?
            .affected_rows();
        Ok(n > 0)
    }

    // ---------- enrich rules ----------

    pub fn list_enrich_rules(&self) -> Result<Vec<EnrichRule>> {
        let mut conn = self.conn()?;
        let rows: Vec<Row> = conn.query(
            "SELECT id, name, kind, matchers_json, match_key, templates_json, mappings_json,
                    write_labels, enabled, priority, created_at, updated_at,
                    lookup_table_id, lookup_table_ids_json, field_templates_json, label_extracts_json, lookup_match_keys_json
             FROM enrich_rules ORDER BY priority ASC, name ASC",
        )?;
        rows.iter().map(map_enrich).collect()
    }

    pub fn get_enrich_rule(&self, id: Uuid) -> Result<Option<EnrichRule>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT id, name, kind, matchers_json, match_key, templates_json, mappings_json,
                    write_labels, enabled, priority, created_at, updated_at,
                    lookup_table_id, lookup_table_ids_json, field_templates_json, label_extracts_json, lookup_match_keys_json
             FROM enrich_rules WHERE id=?",
            positional(vec![v(id.to_string())]),
        )?;
        Ok(row.as_ref().map(map_enrich).transpose()?)
    }

    pub fn upsert_enrich_rule(&self, r: &EnrichRule) -> Result<()> {
        let mut conn = self.conn()?;
        let ids_json = serde_json::to_string(&r.lookup_table_ids)?;
        let legacy_id = r.lookup_table_ids.first().map(|u| u.to_string());
        conn.exec_drop(
            "INSERT INTO enrich_rules (
                id, name, kind, matchers_json, match_key, templates_json, mappings_json,
                write_labels, enabled, priority, created_at, updated_at,
                lookup_table_id, lookup_table_ids_json, field_templates_json, label_extracts_json,
                lookup_match_keys_json
             ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
             ON DUPLICATE KEY UPDATE
               name=VALUES(name), kind=VALUES(kind), matchers_json=VALUES(matchers_json),
               match_key=VALUES(match_key), templates_json=VALUES(templates_json),
               mappings_json=VALUES(mappings_json), write_labels=VALUES(write_labels),
               enabled=VALUES(enabled), priority=VALUES(priority),
               updated_at=VALUES(updated_at), lookup_table_id=VALUES(lookup_table_id),
               lookup_table_ids_json=VALUES(lookup_table_ids_json),
               field_templates_json=VALUES(field_templates_json),
               label_extracts_json=VALUES(label_extracts_json),
               lookup_match_keys_json=VALUES(lookup_match_keys_json)",
            positional(vec![
                v(r.id.to_string()),
                v(r.name.as_str()),
                v(r.kind.as_str()),
                v(serde_json::to_string(&r.matchers)?),
                v(r.match_key.as_str()),
                v(serde_json::to_string(&r.templates)?),
                v(serde_json::to_string(&r.mappings)?),
                v(r.write_labels as i64),
                v(r.enabled as i64),
                v(r.priority),
                v(fmt_dt(r.created_at)),
                v(fmt_dt(r.updated_at)),
                v(legacy_id),
                v(ids_json),
                v(serde_json::to_string(&r.field_templates)?),
                v(serde_json::to_string(&r.label_extracts)?),
                v(serde_json::to_string(&r.lookup_match_keys)?),
            ]),
        )?;
        Ok(())
    }

    pub fn delete_enrich_rule(&self, id: Uuid) -> Result<bool> {
        let mut conn = self.conn()?;
        let n = conn
            .exec_iter(
                "DELETE FROM enrich_rules WHERE id=?",
                positional(vec![v(id.to_string())]),
            )?
            .affected_rows();
        Ok(n > 0)
    }

    // ---------- lookup tables ----------

    pub fn list_lookup_tables(&self) -> Result<Vec<LookupTable>> {
        let mut conn = self.conn()?;
        let rows: Vec<Row> = conn.query(
            "SELECT id, name, description, key_label, rows_json, enabled, created_at, updated_at
             FROM lookup_tables ORDER BY name ASC",
        )?;
        rows.iter().map(map_lookup).collect()
    }

    pub fn lookup_tables_map(&self) -> Result<BTreeMap<Uuid, LookupTable>> {
        Ok(self
            .list_lookup_tables()?
            .into_iter()
            .map(|t| (t.id, t))
            .collect())
    }

    pub fn get_lookup_table(&self, id: Uuid) -> Result<Option<LookupTable>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT id, name, description, key_label, rows_json, enabled, created_at, updated_at
             FROM lookup_tables WHERE id=?",
            positional(vec![v(id.to_string())]),
        )?;
        Ok(row.as_ref().map(map_lookup).transpose()?)
    }

    pub fn upsert_lookup_table(&self, t: &LookupTable) -> Result<()> {
        let mut conn = self.conn()?;
        conn.exec_drop(
            "INSERT INTO lookup_tables (
                id, name, description, key_label, rows_json, enabled, created_at, updated_at
             ) VALUES (?,?,?,?,?,?,?,?)
             ON DUPLICATE KEY UPDATE
               name=VALUES(name), description=VALUES(description),
               key_label=VALUES(key_label), rows_json=VALUES(rows_json),
               enabled=VALUES(enabled), updated_at=VALUES(updated_at)",
            positional(vec![
                v(t.id.to_string()),
                v(t.name.as_str()),
                v(t.description.as_str()),
                v(t.key_label.as_str()),
                v(serde_json::to_string(&t.rows)?),
                v(t.enabled as i64),
                v(fmt_dt(t.created_at)),
                v(fmt_dt(t.updated_at)),
            ]),
        )?;
        Ok(())
    }

    pub fn delete_lookup_table(&self, id: Uuid) -> Result<bool> {
        let mut conn = self.conn()?;
        let n = conn
            .exec_iter(
                "DELETE FROM lookup_tables WHERE id=?",
                positional(vec![v(id.to_string())]),
            )?
            .affected_rows();
        Ok(n > 0)
    }

    pub fn insert_notify_log(&self, log: &NotifyLog) -> Result<()> {
        let mut conn = self.conn()?;
        let transition = match log.transition {
            AlertTransition::BecameFiring => "became_firing",
            AlertTransition::BecameResolved => "became_resolved",
            AlertTransition::Unchanged => "unchanged",
        };
        conn.exec_drop(
            "INSERT INTO notify_logs (id, alert_id, channel_id, transition, success, error, body, created_at)
             VALUES (?,?,?,?,?,?,?,?)",
            positional(vec![
                v(log.id.to_string()),
                v(log.alert_id.to_string()),
                v(log.channel_id.to_string()),
                v(transition),
                v(log.success as i64),
                v(log.error.clone()),
                v(log.body.as_str()),
                v(fmt_dt(log.created_at)),
            ]),
        )?;
        Ok(())
    }

    // ---------- ingress routes ----------

    pub fn list_ingress_routes(&self) -> Result<Vec<IngressRoute>> {
        let mut conn = self.conn()?;
        let rows: Vec<Row> = conn.query(
            "SELECT id, name, kind, token, channel_ids_json, enabled, created_at, updated_at,
                    COALESCE(endpoint, ''), COALESCE(options_json, '{}')
             FROM ingress_routes ORDER BY name",
        )?;
        rows.iter().map(map_ingress).collect()
    }

    pub fn get_ingress_route(&self, id: Uuid) -> Result<Option<IngressRoute>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT id, name, kind, token, channel_ids_json, enabled, created_at, updated_at,
                    COALESCE(endpoint, ''), COALESCE(options_json, '{}')
             FROM ingress_routes WHERE id=?",
            positional(vec![v(id.to_string())]),
        )?;
        Ok(row.as_ref().map(map_ingress).transpose()?)
    }

    pub fn upsert_ingress_route(&self, route: &IngressRoute) -> Result<()> {
        let mut conn = self.conn()?;
        let channel_ids: Vec<String> = route.channel_ids.iter().map(|u| u.to_string()).collect();
        conn.exec_drop(
            "INSERT INTO ingress_routes (id, name, kind, token, channel_ids_json, enabled, created_at, updated_at, endpoint, options_json)
             VALUES (?,?,?,?,?,?,?,?,?,?)
             ON DUPLICATE KEY UPDATE
               name=VALUES(name), kind=VALUES(kind), token=VALUES(token),
               channel_ids_json=VALUES(channel_ids_json), enabled=VALUES(enabled),
               updated_at=VALUES(updated_at), endpoint=VALUES(endpoint),
               options_json=VALUES(options_json)",
            positional(vec![
                v(route.id.to_string()),
                v(route.name.as_str()),
                v(route.kind.as_str()),
                v(route.token.clone()),
                v(serde_json::to_string(&channel_ids)?),
                v(route.enabled as i64),
                v(fmt_dt(route.created_at)),
                v(fmt_dt(route.updated_at)),
                v(route.endpoint.as_str()),
                v(serde_json::to_string(&route.options)?),
            ]),
        )?;
        Ok(())
    }

    /// Legacy MySQL Kafka offsets (unused after consumer-group ingress).
    #[allow(dead_code)]
    pub fn get_kafka_offset(&self, route_id: Uuid, partition: i32) -> Result<Option<i64>> {
        let mut conn = self.conn()?;
        let offset: Option<i64> = conn.exec_first(
            "SELECT next_offset FROM ingress_kafka_offsets WHERE route_id=? AND `partition`=?",
            positional(vec![v(route_id.to_string()), v(partition)]),
        )?;
        Ok(offset)
    }

    /// Legacy MySQL Kafka offsets (unused after consumer-group ingress).
    #[allow(dead_code)]
    pub fn set_kafka_offset(&self, route_id: Uuid, partition: i32, next_offset: i64) -> Result<()> {
        let mut conn = self.conn()?;
        conn.exec_drop(
            "INSERT INTO ingress_kafka_offsets (route_id, `partition`, next_offset)
             VALUES (?,?,?)
             ON DUPLICATE KEY UPDATE next_offset=VALUES(next_offset)",
            positional(vec![
                v(route_id.to_string()),
                v(partition),
                v(next_offset),
            ]),
        )?;
        Ok(())
    }

    pub fn delete_ingress_route(&self, id: Uuid) -> Result<bool> {
        let mut conn = self.conn()?;
        let _ = conn.exec_drop(
            "DELETE FROM ingress_kafka_offsets WHERE route_id=?",
            positional(vec![v(id.to_string())]),
        );
        let n = conn
            .exec_iter(
                "DELETE FROM ingress_routes WHERE id=?",
                positional(vec![v(id.to_string())]),
            )?
            .affected_rows();
        Ok(n > 0)
    }

    pub fn get_kv(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT `value` FROM app_kv WHERE `key`=?",
            positional(vec![v(key)]),
        )?;
        Ok(row.as_ref().and_then(|r| col_str_opt(r, 0)))
    }

    pub fn set_kv(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.conn()?;
        let now = fmt_dt(Utc::now());
        conn.exec_drop(
            "INSERT INTO app_kv (`key`, `value`, updated_at) VALUES (?,?,?)
             ON DUPLICATE KEY UPDATE `value`=VALUES(`value`), updated_at=VALUES(updated_at)",
            positional(vec![
                Value::from(key),
                Value::from(value),
                Value::from(now),
            ]),
        )?;
        Ok(())
    }

    pub fn delete_kv(&self, key: &str) -> Result<()> {
        let mut conn = self.conn()?;
        conn.exec_drop(
            "DELETE FROM app_kv WHERE `key`=?",
            positional(vec![v(key)]),
        )?;
        Ok(())
    }
}

fn map_datasource(row: &Row) -> Result<Datasource> {
    let kind_s = col_str(row, 2)?;
    let kind = DatasourceKind::parse(&kind_s).unwrap_or(DatasourceKind::Prometheus);
    let created = col_str(row, 5)?;
    let updated = col_str(row, 6)?;
    let options_json = col_str_opt(row, 7).unwrap_or_else(|| "{}".into());
    Ok(Datasource {
        id: col_uuid(row, 0)?,
        name: col_str(row, 1)?,
        kind,
        url: col_str(row, 3)?,
        options: labels_from_json(&options_json).unwrap_or_default(),
        enabled: col_i64(row, 4) != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_channel(row: &Row) -> Result<NotifyChannel> {
    let kind_s = col_str(row, 2)?;
    let kind = ChannelKind::parse(&kind_s).unwrap_or(ChannelKind::Webhook);
    let created = col_str(row, 6)?;
    let updated = col_str(row, 7)?;
    let options_json = col_str_opt(row, 8).unwrap_or_else(|| "{}".into());
    Ok(NotifyChannel {
        id: col_uuid(row, 0)?,
        name: col_str(row, 1)?,
        kind,
        url: col_str(row, 3)?,
        secret: col_str_opt(row, 4),
        options: labels_from_json(&options_json).unwrap_or_default(),
        enabled: col_i64(row, 5) != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_rule(row: &Row) -> Result<Rule> {
    let cmp_s = col_str(row, 4)?;
    let sev_s = col_str(row, 8)?;
    let labels_json = col_str(row, 9)?;
    let annotations_json = col_str(row, 10)?;
    let channel_ids_json = col_str(row, 11)?;
    let created = col_str(row, 13)?;
    let updated = col_str(row, 14)?;
    Ok(Rule {
        id: col_uuid(row, 0)?,
        name: col_str(row, 1)?,
        datasource_id: col_uuid(row, 2)?,
        expr: col_str(row, 3)?,
        comparator: Comparator::parse(&cmp_s).unwrap_or(Comparator::Gt),
        threshold: col_f64_opt(row, 5).unwrap_or(0.0),
        for_seconds: col_i64(row, 6) as u64,
        interval_seconds: col_i64(row, 7) as u64,
        severity: Severity::parse(&sev_s).unwrap_or(Severity::Warning),
        labels: labels_from_json(&labels_json).unwrap_or_default(),
        annotations: labels_from_json(&annotations_json).unwrap_or_default(),
        channel_ids: uuid_ids_from_json(&channel_ids_json).unwrap_or_default(),
        enabled: col_i64(row, 12) != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_alert(row: &Row) -> Result<AlertEvent> {
    let status_s = col_str(row, 3)?;
    let sev_s = col_str(row, 4)?;
    let labels_json = col_str(row, 5)?;
    let annotations_json = col_str(row, 6)?;
    let starts = col_str(row, 8)?;
    let ends = col_str_opt(row, 9);
    let pending = col_str_opt(row, 10);
    let last = col_str(row, 11)?;
    Ok(AlertEvent {
        id: col_uuid(row, 0)?,
        rule_id: col_uuid(row, 1)?,
        fingerprint: col_str(row, 2)?,
        status: AlertStatus::parse(&status_s).unwrap_or(AlertStatus::Resolved),
        severity: Severity::parse(&sev_s).unwrap_or(Severity::Warning),
        labels: labels_from_json(&labels_json).unwrap_or_default(),
        annotations: labels_from_json(&annotations_json).unwrap_or_default(),
        value: col_f64_opt(row, 7),
        starts_at: parse_dt(&starts).unwrap_or_else(|_| Utc::now()),
        ends_at: ends.as_deref().and_then(|s| parse_dt(s).ok()),
        pending_since: pending.as_deref().and_then(|s| parse_dt(s).ok()),
        last_evaluated_at: parse_dt(&last).unwrap_or_else(|_| Utc::now()),
        notified_firing: col_i64(row, 12) != 0,
        notified_resolved: col_i64(row, 13) != 0,
    })
}

fn map_enrich(row: &Row) -> Result<EnrichRule> {
    let kind_s = col_str(row, 2)?;
    let matchers_json = col_str(row, 3)?;
    let templates_json = col_str(row, 5)?;
    let mappings_json = col_str(row, 6)?;
    let created = col_str(row, 10)?;
    let updated = col_str(row, 11)?;
    let lookup_table_id = col_str_opt(row, 12);
    let lookup_table_ids_json = col_str_opt(row, 13).unwrap_or_else(|| "[]".into());
    let field_templates_json = col_str_opt(row, 14).unwrap_or_else(|| "{}".into());
    let label_extracts_json = col_str_opt(row, 15).unwrap_or_else(|| "{}".into());
    let lookup_match_keys_json = col_str_opt(row, 16).unwrap_or_else(|| "{}".into());
    let mappings: BTreeMap<String, Labels> =
        serde_json::from_str(&mappings_json).unwrap_or_default();
    let mut lookup_table_ids: Vec<Uuid> = serde_json::from_str::<Vec<String>>(&lookup_table_ids_json)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| Uuid::parse_str(&s).ok())
        .collect();
    if lookup_table_ids.is_empty() {
        if let Some(s) = lookup_table_id {
            if let Ok(u) = Uuid::parse_str(&s) {
                lookup_table_ids.push(u);
            }
        }
    }
    let lookup_match_keys: BTreeMap<String, String> =
        serde_json::from_str(&lookup_match_keys_json).unwrap_or_default();
    Ok(EnrichRule {
        id: col_uuid(row, 0)?,
        name: col_str(row, 1)?,
        kind: EnrichKind::parse(&kind_s).unwrap_or(EnrichKind::AnnotationTemplate),
        matchers: labels_from_json(&matchers_json).unwrap_or_default(),
        match_key: col_str(row, 4).unwrap_or_default(),
        templates: labels_from_json(&templates_json).unwrap_or_default(),
        mappings,
        lookup_table_ids,
        lookup_match_keys,
        field_templates: labels_from_json(&field_templates_json).unwrap_or_default(),
        label_extracts: labels_from_json(&label_extracts_json).unwrap_or_default(),
        write_labels: col_i64(row, 7) != 0,
        enabled: col_i64(row, 8) != 0,
        priority: col_i64(row, 9) as i32,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_lookup(row: &Row) -> Result<LookupTable> {
    let rows_json = col_str(row, 4)?;
    let created = col_str(row, 6)?;
    let updated = col_str(row, 7)?;
    let rows_map: BTreeMap<String, Labels> = serde_json::from_str(&rows_json).unwrap_or_default();
    Ok(LookupTable {
        id: col_uuid(row, 0)?,
        name: col_str(row, 1)?,
        description: col_str(row, 2).unwrap_or_default(),
        key_label: col_str(row, 3).unwrap_or_else(|_| "instance".into()),
        rows: rows_map,
        enabled: col_i64(row, 5) != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_silence(row: &Row) -> Result<Silence> {
    let rule_id = col_str_opt(row, 1);
    let matchers_json = col_str(row, 2)?;
    let starts = col_str(row, 3)?;
    let ends = col_str(row, 4)?;
    let created = col_str(row, 6)?;
    Ok(Silence {
        id: col_uuid(row, 0)?,
        rule_id: rule_id.and_then(|s| Uuid::parse_str(&s).ok()),
        matchers: labels_from_json(&matchers_json).unwrap_or_default(),
        starts_at: parse_dt(&starts).unwrap_or_else(|_| Utc::now()),
        ends_at: parse_dt(&ends).unwrap_or_else(|_| Utc::now()),
        comment: col_str(row, 5).unwrap_or_default(),
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_ingress(row: &Row) -> Result<IngressRoute> {
    let kind_s = col_str(row, 2)?;
    let channel_ids_json = col_str(row, 4)?;
    let created = col_str(row, 6)?;
    let updated = col_str(row, 7)?;
    let endpoint = col_str_opt(row, 8).unwrap_or_default();
    let options_json = col_str_opt(row, 9).unwrap_or_else(|| "{}".into());
    Ok(IngressRoute {
        id: col_uuid(row, 0)?,
        name: col_str(row, 1)?,
        kind: IngressKind::parse(&kind_s).unwrap_or(IngressKind::Generic),
        token: col_str_opt(row, 3),
        endpoint,
        options: labels_from_json(&options_json).unwrap_or_default(),
        channel_ids: uuid_ids_from_json(&channel_ids_json).unwrap_or_default(),
        enabled: col_i64(row, 5) != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_notify_log(row: &Row) -> Result<NotifyLog> {
    let transition_s = col_str(row, 3)?;
    let created = col_str(row, 6)?;
    let body = col_str_opt(row, 7).unwrap_or_default();
    let transition = match transition_s.as_str() {
        "became_firing" => AlertTransition::BecameFiring,
        "became_resolved" => AlertTransition::BecameResolved,
        _ => AlertTransition::Unchanged,
    };
    Ok(NotifyLog {
        id: col_uuid(row, 0)?,
        alert_id: col_uuid(row, 1)?,
        channel_id: col_uuid(row, 2)?,
        transition,
        success: col_i64(row, 4) != 0,
        error: col_str_opt(row, 5),
        body,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
    })
}
