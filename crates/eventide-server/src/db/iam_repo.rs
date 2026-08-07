//! IAM repository: departments, roles, users.

use super::Db;
use crate::iam::{Department, Role, UserAccount};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use mysql::prelude::*;
use mysql::{Params, Row, Value};
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

fn col_uuid(row: &Row, idx: usize) -> Result<Uuid> {
    Uuid::parse_str(&col_str(row, idx)?).map_err(|e| anyhow!(e))
}

impl Db {
    pub fn list_departments(&self) -> Result<Vec<Department>> {
        let mut conn = self.conn()?;
        let rows: Vec<Row> = conn.query(
            "SELECT id, name, parent_id, sort_order, enabled, created_at, updated_at
             FROM departments ORDER BY sort_order, name",
        )?;
        rows.iter().map(map_department).collect()
    }

    pub fn get_department(&self, id: Uuid) -> Result<Option<Department>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT id, name, parent_id, sort_order, enabled, created_at, updated_at
             FROM departments WHERE id=?",
            positional(vec![Value::from(id.to_string())]),
        )?;
        Ok(row.as_ref().map(map_department).transpose()?)
    }

    pub fn upsert_department(&self, d: &Department) -> Result<()> {
        let mut conn = self.conn()?;
        conn.exec_drop(
            "INSERT INTO departments (id, name, parent_id, sort_order, enabled, created_at, updated_at)
             VALUES (?,?,?,?,?,?,?)
             ON DUPLICATE KEY UPDATE
               name=VALUES(name), parent_id=VALUES(parent_id), sort_order=VALUES(sort_order),
               enabled=VALUES(enabled), updated_at=VALUES(updated_at)",
            positional(vec![
                Value::from(d.id.to_string()),
                Value::from(d.name.as_str()),
                Value::from(d.parent_id.map(|x| x.to_string())),
                Value::from(d.sort_order),
                Value::from(d.enabled as i64),
                Value::from(fmt_dt(d.created_at)),
                Value::from(fmt_dt(d.updated_at)),
            ]),
        )?;
        Ok(())
    }

    pub fn delete_department(&self, id: Uuid) -> Result<bool> {
        let mut conn = self.conn()?;
        let children: i64 = conn
            .exec_first(
                "SELECT COUNT(*) FROM departments WHERE parent_id=?",
                positional(vec![Value::from(id.to_string())]),
            )?
            .unwrap_or(0);
        if children > 0 {
            return Err(anyhow!("部门下仍有子部门，无法删除"));
        }
        let users: i64 = conn
            .exec_first(
                "SELECT COUNT(*) FROM users WHERE department_id=?",
                positional(vec![Value::from(id.to_string())]),
            )?
            .unwrap_or(0);
        if users > 0 {
            return Err(anyhow!("部门下仍有用户，无法删除"));
        }
        let n = conn
            .exec_iter(
                "DELETE FROM departments WHERE id=?",
                positional(vec![Value::from(id.to_string())]),
            )?
            .affected_rows();
        Ok(n > 0)
    }

    pub fn list_roles(&self) -> Result<Vec<Role>> {
        let mut conn = self.conn()?;
        let rows: Vec<Row> = conn.query(
            "SELECT id, name, description, permissions_json, is_system, created_at, updated_at
             FROM roles ORDER BY name",
        )?;
        rows.iter().map(map_role).collect()
    }

    pub fn get_role(&self, id: Uuid) -> Result<Option<Role>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT id, name, description, permissions_json, is_system, created_at, updated_at
             FROM roles WHERE id=?",
            positional(vec![Value::from(id.to_string())]),
        )?;
        Ok(row.as_ref().map(map_role).transpose()?)
    }

    pub fn upsert_role(&self, r: &Role) -> Result<()> {
        let mut conn = self.conn()?;
        conn.exec_drop(
            "INSERT INTO roles (id, name, description, permissions_json, is_system, created_at, updated_at)
             VALUES (?,?,?,?,?,?,?)
             ON DUPLICATE KEY UPDATE
               name=VALUES(name), description=VALUES(description),
               permissions_json=VALUES(permissions_json), updated_at=VALUES(updated_at)",
            positional(vec![
                Value::from(r.id.to_string()),
                Value::from(r.name.as_str()),
                Value::from(r.description.as_str()),
                Value::from(serde_json::to_string(&r.permissions)?),
                Value::from(r.is_system as i64),
                Value::from(fmt_dt(r.created_at)),
                Value::from(fmt_dt(r.updated_at)),
            ]),
        )?;
        Ok(())
    }

    pub fn delete_role(&self, id: Uuid) -> Result<bool> {
        let role = self
            .get_role(id)?
            .ok_or_else(|| anyhow!("角色不存在"))?;
        if role.is_system {
            return Err(anyhow!("系统角色不可删除"));
        }
        let users = self.list_users()?;
        if users.iter().any(|u| u.role_ids.contains(&id)) {
            return Err(anyhow!("仍有用户使用该角色，无法删除"));
        }
        let mut conn = self.conn()?;
        let n = conn
            .exec_iter(
                "DELETE FROM roles WHERE id=?",
                positional(vec![Value::from(id.to_string())]),
            )?
            .affected_rows();
        Ok(n > 0)
    }

    pub fn list_users(&self) -> Result<Vec<UserAccount>> {
        let mut conn = self.conn()?;
        let rows: Vec<Row> = conn.query(
            "SELECT id, username, display_name, password_hash, department_id, role_ids_json,
                    enabled, created_at, updated_at, password_changed_at
             FROM users ORDER BY username",
        )?;
        rows.iter().map(map_user).collect()
    }

    pub fn get_user(&self, id: Uuid) -> Result<Option<UserAccount>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT id, username, display_name, password_hash, department_id, role_ids_json,
                    enabled, created_at, updated_at, password_changed_at
             FROM users WHERE id=?",
            positional(vec![Value::from(id.to_string())]),
        )?;
        Ok(row.as_ref().map(map_user).transpose()?)
    }

    pub fn get_user_by_username(&self, username: &str) -> Result<Option<UserAccount>> {
        let mut conn = self.conn()?;
        let row: Option<Row> = conn.exec_first(
            "SELECT id, username, display_name, password_hash, department_id, role_ids_json,
                    enabled, created_at, updated_at, password_changed_at
             FROM users WHERE username=?",
            positional(vec![Value::from(username)]),
        )?;
        Ok(row.as_ref().map(map_user).transpose()?)
    }

    pub fn upsert_user(&self, u: &UserAccount) -> Result<()> {
        let mut conn = self.conn()?;
        conn.exec_drop(
            "INSERT INTO users (id, username, display_name, password_hash, department_id,
                                role_ids_json, enabled, created_at, updated_at, password_changed_at)
             VALUES (?,?,?,?,?,?,?,?,?,?)
             ON DUPLICATE KEY UPDATE
               username=VALUES(username), display_name=VALUES(display_name),
               password_hash=VALUES(password_hash), department_id=VALUES(department_id),
               role_ids_json=VALUES(role_ids_json), enabled=VALUES(enabled),
               updated_at=VALUES(updated_at),
               password_changed_at=VALUES(password_changed_at)",
            positional(vec![
                Value::from(u.id.to_string()),
                Value::from(u.username.as_str()),
                Value::from(u.display_name.as_str()),
                Value::from(u.password_hash.as_str()),
                Value::from(u.department_id.map(|x| x.to_string())),
                Value::from(serde_json::to_string(
                    &u.role_ids
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>(),
                )?),
                Value::from(u.enabled as i64),
                Value::from(fmt_dt(u.created_at)),
                Value::from(fmt_dt(u.updated_at)),
                Value::from(fmt_dt(u.password_changed_at)),
            ]),
        )?;
        Ok(())
    }

    pub fn delete_user(&self, id: Uuid) -> Result<bool> {
        let mut conn = self.conn()?;
        let n = conn
            .exec_iter(
                "DELETE FROM users WHERE id=?",
                positional(vec![Value::from(id.to_string())]),
            )?
            .affected_rows();
        Ok(n > 0)
    }

    pub fn permissions_for_roles(&self, role_ids: &[Uuid]) -> Result<Vec<String>> {
        if role_ids.is_empty() {
            return Ok(vec![]);
        }
        let mut set = std::collections::BTreeSet::new();
        for id in role_ids {
            if let Some(role) = self.get_role(*id)? {
                for p in role.permissions {
                    set.insert(p);
                }
            }
        }
        Ok(set.into_iter().collect())
    }

    pub fn count_star_admins(&self) -> Result<i64> {
        let users = self.list_users()?;
        let mut n = 0i64;
        for u in users {
            if !u.enabled {
                continue;
            }
            let perms = self.permissions_for_roles(&u.role_ids)?;
            if crate::iam::has_perm(&perms, "*") {
                n += 1;
            }
        }
        Ok(n)
    }
}

fn map_department(row: &Row) -> Result<Department> {
    let parent = col_str_opt(row, 2);
    let created = col_str(row, 5)?;
    let updated = col_str(row, 6)?;
    Ok(Department {
        id: col_uuid(row, 0)?,
        name: col_str(row, 1)?,
        parent_id: parent
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|e| anyhow!(e))?,
        sort_order: col_i64(row, 3),
        enabled: col_i64(row, 4) != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_role(row: &Row) -> Result<Role> {
    let perms_json = col_str(row, 3)?;
    let created = col_str(row, 5)?;
    let updated = col_str(row, 6)?;
    let permissions: Vec<String> = serde_json::from_str(&perms_json).unwrap_or_default();
    Ok(Role {
        id: col_uuid(row, 0)?,
        name: col_str(row, 1)?,
        description: col_str(row, 2).unwrap_or_default(),
        permissions,
        is_system: col_i64(row, 4) != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_user(row: &Row) -> Result<UserAccount> {
    let dept = col_str_opt(row, 4);
    let roles_json = col_str(row, 5)?;
    let created = col_str(row, 7)?;
    let updated = col_str(row, 8)?;
    let created_at = parse_dt(&created).unwrap_or_else(|_| Utc::now());
    let updated_at = parse_dt(&updated).unwrap_or_else(|_| Utc::now());
    let password_changed_at = col_str_opt(row, 9)
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| parse_dt(s).ok())
        .unwrap_or(created_at);
    let role_ids = uuid_ids_from_json(&roles_json).unwrap_or_default();
    Ok(UserAccount {
        id: col_uuid(row, 0)?,
        username: col_str(row, 1)?,
        display_name: col_str(row, 2).unwrap_or_default(),
        password_hash: col_str(row, 3)?,
        department_id: dept
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|e| anyhow!(e))?,
        role_ids,
        enabled: col_i64(row, 6) != 0,
        created_at,
        updated_at,
        password_changed_at,
    })
}
