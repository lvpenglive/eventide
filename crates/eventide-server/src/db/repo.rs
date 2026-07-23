//! Repository helpers mapping SQLite rows to core models.

use super::Db;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use eventide_core::*;
use rusqlite::{params, OptionalExtension, Row};
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

fn labels_from_json(s: &str) -> Result<Labels> {
    Ok(serde_json::from_str(s).unwrap_or_default())
}

fn uuid_ids_from_json(s: &str) -> Result<Vec<Uuid>> {
    let raw: Vec<String> = serde_json::from_str(s).unwrap_or_default();
    raw.into_iter()
        .map(|x| Uuid::parse_str(&x).map_err(|e| anyhow!(e)))
        .collect()
}

impl Db {
    // ---------- datasources ----------

    pub fn list_datasources(&self) -> Result<Vec<Datasource>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, url, enabled, created_at, updated_at,
                    COALESCE(options_json, '{}') FROM datasources ORDER BY name",
        )?;
        let rows = stmt.query_map([], map_datasource)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_datasource(&self, id: Uuid) -> Result<Option<Datasource>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, name, kind, url, enabled, created_at, updated_at,
                    COALESCE(options_json, '{}') FROM datasources WHERE id=?1",
            params![id.to_string()],
            map_datasource,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn upsert_datasource(&self, ds: &Datasource) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO datasources (id, name, kind, url, enabled, created_at, updated_at, options_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, kind=excluded.kind, url=excluded.url,
               enabled=excluded.enabled, updated_at=excluded.updated_at,
               options_json=excluded.options_json",
            params![
                ds.id.to_string(),
                ds.name,
                ds.kind.as_str(),
                ds.url,
                ds.enabled as i64,
                fmt_dt(ds.created_at),
                fmt_dt(ds.updated_at),
                serde_json::to_string(&ds.options)?,
            ],
        )?;
        Ok(())
    }

    pub fn delete_datasource(&self, id: Uuid) -> Result<bool> {
        let conn = self.lock()?;
        let n = conn.execute(
            "DELETE FROM datasources WHERE id=?1",
            params![id.to_string()],
        )?;
        Ok(n > 0)
    }

    // ---------- channels ----------

    pub fn list_channels(&self) -> Result<Vec<NotifyChannel>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, url, secret, enabled, created_at, updated_at FROM notify_channels ORDER BY name",
        )?;
        let rows = stmt.query_map([], map_channel)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_channel(&self, id: Uuid) -> Result<Option<NotifyChannel>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, name, kind, url, secret, enabled, created_at, updated_at FROM notify_channels WHERE id=?1",
            params![id.to_string()],
            map_channel,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn upsert_channel(&self, ch: &NotifyChannel) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO notify_channels (id, name, kind, url, secret, enabled, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, kind=excluded.kind, url=excluded.url, secret=excluded.secret,
               enabled=excluded.enabled, updated_at=excluded.updated_at",
            params![
                ch.id.to_string(),
                ch.name,
                ch.kind.as_str(),
                ch.url,
                ch.secret,
                ch.enabled as i64,
                fmt_dt(ch.created_at),
                fmt_dt(ch.updated_at),
            ],
        )?;
        Ok(())
    }

    pub fn delete_channel(&self, id: Uuid) -> Result<bool> {
        let conn = self.lock()?;
        let n = conn.execute(
            "DELETE FROM notify_channels WHERE id=?1",
            params![id.to_string()],
        )?;
        Ok(n > 0)
    }

    // ---------- rules ----------

    pub fn list_rules(&self) -> Result<Vec<Rule>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, datasource_id, expr, comparator, threshold, for_seconds, interval_seconds,
                    severity, labels_json, annotations_json, channel_ids_json, enabled, created_at, updated_at
             FROM rules ORDER BY name",
        )?;
        let rows = stmt.query_map([], map_rule)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_enabled_rules_due(&self, now: DateTime<Utc>) -> Result<Vec<Rule>> {
        let rules = self.list_rules()?;
        let conn = self.lock()?;
        let mut due = Vec::new();
        for rule in rules {
            if !rule.enabled {
                continue;
            }
            let last: Option<String> = conn
                .query_row(
                    "SELECT last_run_at FROM rules WHERE id=?1",
                    params![rule.id.to_string()],
                    |r| r.get(0),
                )
                .optional()?
                .flatten();
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
        let conn = self.lock()?;
        conn.execute(
            "UPDATE rules SET last_run_at=?1 WHERE id=?2",
            params![fmt_dt(at), id.to_string()],
        )?;
        Ok(())
    }

    pub fn get_rule(&self, id: Uuid) -> Result<Option<Rule>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, name, datasource_id, expr, comparator, threshold, for_seconds, interval_seconds,
                    severity, labels_json, annotations_json, channel_ids_json, enabled, created_at, updated_at
             FROM rules WHERE id=?1",
            params![id.to_string()],
            map_rule,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn upsert_rule(&self, rule: &Rule) -> Result<()> {
        let conn = self.lock()?;
        let channel_ids: Vec<String> = rule.channel_ids.iter().map(|u| u.to_string()).collect();
        conn.execute(
            "INSERT INTO rules (
                id, name, datasource_id, expr, comparator, threshold, for_seconds, interval_seconds,
                severity, labels_json, annotations_json, channel_ids_json, enabled, created_at, updated_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, datasource_id=excluded.datasource_id, expr=excluded.expr,
               comparator=excluded.comparator, threshold=excluded.threshold,
               for_seconds=excluded.for_seconds, interval_seconds=excluded.interval_seconds,
               severity=excluded.severity, labels_json=excluded.labels_json,
               annotations_json=excluded.annotations_json, channel_ids_json=excluded.channel_ids_json,
               enabled=excluded.enabled, updated_at=excluded.updated_at",
            params![
                rule.id.to_string(),
                rule.name,
                rule.datasource_id.to_string(),
                rule.expr,
                rule.comparator.as_str(),
                rule.threshold,
                rule.for_seconds as i64,
                rule.interval_seconds as i64,
                rule.severity.as_str(),
                serde_json::to_string(&rule.labels)?,
                serde_json::to_string(&rule.annotations)?,
                serde_json::to_string(&channel_ids)?,
                rule.enabled as i64,
                fmt_dt(rule.created_at),
                fmt_dt(rule.updated_at),
            ],
        )?;
        Ok(())
    }

    pub fn delete_rule(&self, id: Uuid) -> Result<bool> {
        let conn = self.lock()?;
        let n = conn.execute("DELETE FROM rules WHERE id=?1", params![id.to_string()])?;
        Ok(n > 0)
    }

    // ---------- alert events ----------

    pub fn list_alerts(&self, status: Option<&str>) -> Result<Vec<AlertEvent>> {
        let conn = self.lock()?;
        if let Some(st) = status {
            let mut stmt = conn.prepare(
                "SELECT id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                        value, starts_at, ends_at, pending_since, last_evaluated_at,
                        notified_firing, notified_resolved
                 FROM alert_events WHERE status=?1 ORDER BY last_evaluated_at DESC LIMIT 500",
            )?;
            let rows = stmt.query_map(params![st], map_alert)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Into::into)
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                        value, starts_at, ends_at, pending_since, last_evaluated_at,
                        notified_firing, notified_resolved
                 FROM alert_events ORDER BY last_evaluated_at DESC LIMIT 500",
            )?;
            let rows = stmt.query_map([], map_alert)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Into::into)
        }
    }

    pub fn get_alert_by_fingerprint(&self, fp: &str) -> Result<Option<AlertEvent>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                    value, starts_at, ends_at, pending_since, last_evaluated_at,
                    notified_firing, notified_resolved
             FROM alert_events WHERE fingerprint=?1",
            params![fp],
            map_alert,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_alert(&self, id: Uuid) -> Result<Option<AlertEvent>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                    value, starts_at, ends_at, pending_since, last_evaluated_at,
                    notified_firing, notified_resolved
             FROM alert_events WHERE id=?1",
            params![id.to_string()],
            map_alert,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_notify_logs_for_alert(&self, alert_id: Uuid) -> Result<Vec<NotifyLog>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, alert_id, channel_id, transition, success, error, created_at
             FROM notify_logs WHERE alert_id=?1 ORDER BY created_at DESC LIMIT 100",
        )?;
        let rows = stmt.query_map(params![alert_id.to_string()], map_notify_log)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn alerts_for_rule(&self, rule_id: Uuid) -> Result<BTreeMap<String, AlertEvent>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                    value, starts_at, ends_at, pending_since, last_evaluated_at,
                    notified_firing, notified_resolved
             FROM alert_events WHERE rule_id=?1",
        )?;
        let rows = stmt.query_map(params![rule_id.to_string()], map_alert)?;
        let mut map = BTreeMap::new();
        for row in rows {
            let ev = row?;
            map.insert(ev.fingerprint.clone(), ev);
        }
        Ok(map)
    }

    pub fn upsert_alert(&self, ev: &AlertEvent) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO alert_events (
                id, rule_id, fingerprint, status, severity, labels_json, annotations_json,
                value, starts_at, ends_at, pending_since, last_evaluated_at,
                notified_firing, notified_resolved
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
             ON CONFLICT(fingerprint) DO UPDATE SET
               id=excluded.id, status=excluded.status, severity=excluded.severity,
               labels_json=excluded.labels_json, annotations_json=excluded.annotations_json,
               value=excluded.value, starts_at=excluded.starts_at, ends_at=excluded.ends_at,
               pending_since=excluded.pending_since, last_evaluated_at=excluded.last_evaluated_at,
               notified_firing=excluded.notified_firing, notified_resolved=excluded.notified_resolved",
            params![
                ev.id.to_string(),
                ev.rule_id.to_string(),
                ev.fingerprint,
                ev.status.as_str(),
                ev.severity.as_str(),
                serde_json::to_string(&ev.labels)?,
                serde_json::to_string(&ev.annotations)?,
                ev.value,
                fmt_dt(ev.starts_at),
                ev.ends_at.map(fmt_dt),
                ev.pending_since.map(fmt_dt),
                fmt_dt(ev.last_evaluated_at),
                ev.notified_firing as i64,
                ev.notified_resolved as i64,
            ],
        )?;
        Ok(())
    }

    // ---------- silences ----------

    pub fn list_silences(&self) -> Result<Vec<Silence>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, rule_id, matchers_json, starts_at, ends_at, comment, created_at FROM silences ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], map_silence)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn active_silences(&self, now: DateTime<Utc>) -> Result<Vec<Silence>> {
        Ok(self
            .list_silences()?
            .into_iter()
            .filter(|s| now >= s.starts_at && now < s.ends_at)
            .collect())
    }

    pub fn upsert_silence(&self, s: &Silence) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO silences (id, rule_id, matchers_json, starts_at, ends_at, comment, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(id) DO UPDATE SET
               rule_id=excluded.rule_id, matchers_json=excluded.matchers_json,
               starts_at=excluded.starts_at, ends_at=excluded.ends_at, comment=excluded.comment",
            params![
                s.id.to_string(),
                s.rule_id.map(|u| u.to_string()),
                serde_json::to_string(&s.matchers)?,
                fmt_dt(s.starts_at),
                fmt_dt(s.ends_at),
                s.comment,
                fmt_dt(s.created_at),
            ],
        )?;
        Ok(())
    }

    pub fn delete_silence(&self, id: Uuid) -> Result<bool> {
        let conn = self.lock()?;
        let n = conn.execute("DELETE FROM silences WHERE id=?1", params![id.to_string()])?;
        Ok(n > 0)
    }

    // ---------- enrich rules ----------

    pub fn list_enrich_rules(&self) -> Result<Vec<EnrichRule>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, matchers_json, match_key, templates_json, mappings_json,
                    write_labels, enabled, priority, created_at, updated_at,
                    lookup_table_id
             FROM enrich_rules ORDER BY priority ASC, name ASC",
        )?;
        let rows = stmt.query_map([], map_enrich)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_enrich_rule(&self, id: Uuid) -> Result<Option<EnrichRule>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, name, kind, matchers_json, match_key, templates_json, mappings_json,
                    write_labels, enabled, priority, created_at, updated_at,
                    lookup_table_id
             FROM enrich_rules WHERE id=?1",
            params![id.to_string()],
            map_enrich,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn upsert_enrich_rule(&self, r: &EnrichRule) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO enrich_rules (
                id, name, kind, matchers_json, match_key, templates_json, mappings_json,
                write_labels, enabled, priority, created_at, updated_at, lookup_table_id
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, kind=excluded.kind, matchers_json=excluded.matchers_json,
               match_key=excluded.match_key, templates_json=excluded.templates_json,
               mappings_json=excluded.mappings_json, write_labels=excluded.write_labels,
               enabled=excluded.enabled, priority=excluded.priority,
               updated_at=excluded.updated_at, lookup_table_id=excluded.lookup_table_id",
            params![
                r.id.to_string(),
                r.name,
                r.kind.as_str(),
                serde_json::to_string(&r.matchers)?,
                r.match_key,
                serde_json::to_string(&r.templates)?,
                serde_json::to_string(&r.mappings)?,
                r.write_labels as i64,
                r.enabled as i64,
                r.priority,
                fmt_dt(r.created_at),
                fmt_dt(r.updated_at),
                r.lookup_table_id.map(|u| u.to_string()),
            ],
        )?;
        Ok(())
    }

    pub fn delete_enrich_rule(&self, id: Uuid) -> Result<bool> {
        let conn = self.lock()?;
        let n = conn.execute(
            "DELETE FROM enrich_rules WHERE id=?1",
            params![id.to_string()],
        )?;
        Ok(n > 0)
    }

    // ---------- lookup tables ----------

    pub fn list_lookup_tables(&self) -> Result<Vec<LookupTable>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, key_label, rows_json, enabled, created_at, updated_at
             FROM lookup_tables ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], map_lookup)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn lookup_tables_map(&self) -> Result<BTreeMap<Uuid, LookupTable>> {
        Ok(self
            .list_lookup_tables()?
            .into_iter()
            .map(|t| (t.id, t))
            .collect())
    }

    pub fn get_lookup_table(&self, id: Uuid) -> Result<Option<LookupTable>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, name, description, key_label, rows_json, enabled, created_at, updated_at
             FROM lookup_tables WHERE id=?1",
            params![id.to_string()],
            map_lookup,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn upsert_lookup_table(&self, t: &LookupTable) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO lookup_tables (
                id, name, description, key_label, rows_json, enabled, created_at, updated_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, description=excluded.description,
               key_label=excluded.key_label, rows_json=excluded.rows_json,
               enabled=excluded.enabled, updated_at=excluded.updated_at",
            params![
                t.id.to_string(),
                t.name,
                t.description,
                t.key_label,
                serde_json::to_string(&t.rows)?,
                t.enabled as i64,
                fmt_dt(t.created_at),
                fmt_dt(t.updated_at),
            ],
        )?;
        Ok(())
    }

    pub fn delete_lookup_table(&self, id: Uuid) -> Result<bool> {
        let conn = self.lock()?;
        let n = conn.execute(
            "DELETE FROM lookup_tables WHERE id=?1",
            params![id.to_string()],
        )?;
        Ok(n > 0)
    }

    pub fn insert_notify_log(&self, log: &NotifyLog) -> Result<()> {
        let conn = self.lock()?;
        let transition = match log.transition {
            AlertTransition::BecameFiring => "became_firing",
            AlertTransition::BecameResolved => "became_resolved",
            AlertTransition::Unchanged => "unchanged",
        };
        conn.execute(
            "INSERT INTO notify_logs (id, alert_id, channel_id, transition, success, error, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                log.id.to_string(),
                log.alert_id.to_string(),
                log.channel_id.to_string(),
                transition,
                log.success as i64,
                log.error,
                fmt_dt(log.created_at),
            ],
        )?;
        Ok(())
    }

    // ---------- ingress routes ----------

    pub fn list_ingress_routes(&self) -> Result<Vec<IngressRoute>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, token, channel_ids_json, enabled, created_at, updated_at,
                    COALESCE(endpoint, ''), COALESCE(options_json, '{}')
             FROM ingress_routes ORDER BY name",
        )?;
        let rows = stmt.query_map([], map_ingress)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_ingress_route(&self, id: Uuid) -> Result<Option<IngressRoute>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, name, kind, token, channel_ids_json, enabled, created_at, updated_at,
                    COALESCE(endpoint, ''), COALESCE(options_json, '{}')
             FROM ingress_routes WHERE id=?1",
            params![id.to_string()],
            map_ingress,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn upsert_ingress_route(&self, route: &IngressRoute) -> Result<()> {
        let conn = self.lock()?;
        let channel_ids: Vec<String> = route.channel_ids.iter().map(|u| u.to_string()).collect();
        conn.execute(
            "INSERT INTO ingress_routes (id, name, kind, token, channel_ids_json, enabled, created_at, updated_at, endpoint, options_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, kind=excluded.kind, token=excluded.token,
               channel_ids_json=excluded.channel_ids_json, enabled=excluded.enabled,
               updated_at=excluded.updated_at, endpoint=excluded.endpoint,
               options_json=excluded.options_json",
            params![
                route.id.to_string(),
                route.name,
                route.kind.as_str(),
                route.token,
                serde_json::to_string(&channel_ids)?,
                route.enabled as i64,
                fmt_dt(route.created_at),
                fmt_dt(route.updated_at),
                route.endpoint,
                serde_json::to_string(&route.options)?,
            ],
        )?;
        Ok(())
    }

    pub fn get_kafka_offset(&self, route_id: Uuid, partition: i32) -> Result<Option<i64>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT next_offset FROM ingress_kafka_offsets WHERE route_id=?1 AND partition=?2",
            params![route_id.to_string(), partition],
            |r| r.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn set_kafka_offset(&self, route_id: Uuid, partition: i32, next_offset: i64) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO ingress_kafka_offsets (route_id, partition, next_offset)
             VALUES (?1,?2,?3)
             ON CONFLICT(route_id, partition) DO UPDATE SET next_offset=excluded.next_offset",
            params![route_id.to_string(), partition, next_offset],
        )?;
        Ok(())
    }

    pub fn delete_ingress_route(&self, id: Uuid) -> Result<bool> {
        let conn = self.lock()?;
        let _ = conn.execute(
            "DELETE FROM ingress_kafka_offsets WHERE route_id=?1",
            params![id.to_string()],
        );
        let n = conn.execute(
            "DELETE FROM ingress_routes WHERE id=?1",
            params![id.to_string()],
        )?;
        Ok(n > 0)
    }
}

fn map_datasource(row: &Row<'_>) -> rusqlite::Result<Datasource> {
    let kind_s: String = row.get(2)?;
    let kind = DatasourceKind::parse(&kind_s).unwrap_or(DatasourceKind::Prometheus);
    let created: String = row.get(5)?;
    let updated: String = row.get(6)?;
    let options_json: String = row.get(7).unwrap_or_else(|_| "{}".into());
    Ok(Datasource {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: row.get(1)?,
        kind,
        url: row.get(3)?,
        options: labels_from_json(&options_json).unwrap_or_default(),
        enabled: row.get::<_, i64>(4)? != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_channel(row: &Row<'_>) -> rusqlite::Result<NotifyChannel> {
    let kind_s: String = row.get(2)?;
    let kind = ChannelKind::parse(&kind_s).unwrap_or(ChannelKind::Webhook);
    let created: String = row.get(6)?;
    let updated: String = row.get(7)?;
    Ok(NotifyChannel {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: row.get(1)?,
        kind,
        url: row.get(3)?,
        secret: row.get(4)?,
        enabled: row.get::<_, i64>(5)? != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_rule(row: &Row<'_>) -> rusqlite::Result<Rule> {
    let cmp_s: String = row.get(4)?;
    let sev_s: String = row.get(8)?;
    let labels_json: String = row.get(9)?;
    let annotations_json: String = row.get(10)?;
    let channel_ids_json: String = row.get(11)?;
    let created: String = row.get(13)?;
    let updated: String = row.get(14)?;

    Ok(Rule {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: row.get(1)?,
        datasource_id: Uuid::parse_str(&row.get::<_, String>(2)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?,
        expr: row.get(3)?,
        comparator: Comparator::parse(&cmp_s).unwrap_or(Comparator::Gt),
        threshold: row.get(5)?,
        for_seconds: row.get::<_, i64>(6)? as u64,
        interval_seconds: row.get::<_, i64>(7)? as u64,
        severity: Severity::parse(&sev_s).unwrap_or(Severity::Warning),
        labels: labels_from_json(&labels_json).unwrap_or_default(),
        annotations: labels_from_json(&annotations_json).unwrap_or_default(),
        channel_ids: uuid_ids_from_json(&channel_ids_json).unwrap_or_default(),
        enabled: row.get::<_, i64>(12)? != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_alert(row: &Row<'_>) -> rusqlite::Result<AlertEvent> {
    let status_s: String = row.get(3)?;
    let sev_s: String = row.get(4)?;
    let labels_json: String = row.get(5)?;
    let annotations_json: String = row.get(6)?;
    let starts: String = row.get(8)?;
    let ends: Option<String> = row.get(9)?;
    let pending: Option<String> = row.get(10)?;
    let last: String = row.get(11)?;

    Ok(AlertEvent {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        rule_id: Uuid::parse_str(&row.get::<_, String>(1)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        fingerprint: row.get(2)?,
        status: AlertStatus::parse(&status_s).unwrap_or(AlertStatus::Resolved),
        severity: Severity::parse(&sev_s).unwrap_or(Severity::Warning),
        labels: labels_from_json(&labels_json).unwrap_or_default(),
        annotations: labels_from_json(&annotations_json).unwrap_or_default(),
        value: row.get(7)?,
        starts_at: parse_dt(&starts).unwrap_or_else(|_| Utc::now()),
        ends_at: ends.as_deref().and_then(|s| parse_dt(s).ok()),
        pending_since: pending.as_deref().and_then(|s| parse_dt(s).ok()),
        last_evaluated_at: parse_dt(&last).unwrap_or_else(|_| Utc::now()),
        notified_firing: row.get::<_, i64>(12)? != 0,
        notified_resolved: row.get::<_, i64>(13)? != 0,
    })
}

fn map_enrich(row: &Row<'_>) -> rusqlite::Result<EnrichRule> {
    let kind_s: String = row.get(2)?;
    let matchers_json: String = row.get(3)?;
    let templates_json: String = row.get(5)?;
    let mappings_json: String = row.get(6)?;
    let created: String = row.get(10)?;
    let updated: String = row.get(11)?;
    let lookup_table_id: Option<String> = row.get(12).unwrap_or(None);
    let mappings: BTreeMap<String, Labels> =
        serde_json::from_str(&mappings_json).unwrap_or_default();
    Ok(EnrichRule {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: row.get(1)?,
        kind: EnrichKind::parse(&kind_s).unwrap_or(EnrichKind::AnnotationTemplate),
        matchers: labels_from_json(&matchers_json).unwrap_or_default(),
        match_key: row.get(4)?,
        templates: labels_from_json(&templates_json).unwrap_or_default(),
        mappings,
        lookup_table_id: lookup_table_id.and_then(|s| Uuid::parse_str(&s).ok()),
        write_labels: row.get::<_, i64>(7)? != 0,
        enabled: row.get::<_, i64>(8)? != 0,
        priority: row.get(9)?,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_lookup(row: &Row<'_>) -> rusqlite::Result<LookupTable> {
    let rows_json: String = row.get(4)?;
    let created: String = row.get(6)?;
    let updated: String = row.get(7)?;
    let rows: BTreeMap<String, Labels> = serde_json::from_str(&rows_json).unwrap_or_default();
    Ok(LookupTable {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: row.get(1)?,
        description: row.get(2)?,
        key_label: row.get(3)?,
        rows,
        enabled: row.get::<_, i64>(5)? != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_silence(row: &Row<'_>) -> rusqlite::Result<Silence> {
    let rule_id: Option<String> = row.get(1)?;
    let matchers_json: String = row.get(2)?;
    let starts: String = row.get(3)?;
    let ends: String = row.get(4)?;
    let created: String = row.get(6)?;
    Ok(Silence {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        rule_id: rule_id.and_then(|s| Uuid::parse_str(&s).ok()),
        matchers: labels_from_json(&matchers_json).unwrap_or_default(),
        starts_at: parse_dt(&starts).unwrap_or_else(|_| Utc::now()),
        ends_at: parse_dt(&ends).unwrap_or_else(|_| Utc::now()),
        comment: row.get(5)?,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_ingress(row: &Row<'_>) -> rusqlite::Result<IngressRoute> {
    let kind_s: String = row.get(2)?;
    let channel_ids_json: String = row.get(4)?;
    let created: String = row.get(6)?;
    let updated: String = row.get(7)?;
    let endpoint: String = row.get(8).unwrap_or_default();
    let options_json: String = row.get(9).unwrap_or_else(|_| "{}".into());
    Ok(IngressRoute {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: row.get(1)?,
        kind: IngressKind::parse(&kind_s).unwrap_or(IngressKind::Generic),
        token: row.get(3)?,
        endpoint,
        options: labels_from_json(&options_json).unwrap_or_default(),
        channel_ids: uuid_ids_from_json(&channel_ids_json).unwrap_or_default(),
        enabled: row.get::<_, i64>(5)? != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_notify_log(row: &Row<'_>) -> rusqlite::Result<NotifyLog> {
    let transition_s: String = row.get(3)?;
    let created: String = row.get(6)?;
    let transition = match transition_s.as_str() {
        "became_firing" => AlertTransition::BecameFiring,
        "became_resolved" => AlertTransition::BecameResolved,
        _ => AlertTransition::Unchanged,
    };
    Ok(NotifyLog {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        alert_id: Uuid::parse_str(&row.get::<_, String>(1)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?,
        channel_id: Uuid::parse_str(&row.get::<_, String>(2)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
        })?,
        transition,
        success: row.get::<_, i64>(4)? != 0,
        error: row.get(5)?,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
    })
}
