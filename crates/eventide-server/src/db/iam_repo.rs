//! IAM repository: departments, roles, users.

use super::Db;
use crate::iam::{Department, Role, UserAccount};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, Row};
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

fn uuid_ids_from_json(s: &str) -> Result<Vec<Uuid>> {
    let raw: Vec<String> = serde_json::from_str(s).unwrap_or_default();
    raw.into_iter()
        .map(|x| Uuid::parse_str(&x).map_err(|e| anyhow!(e)))
        .collect()
}

impl Db {
    pub fn list_departments(&self) -> Result<Vec<Department>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, sort_order, enabled, created_at, updated_at
             FROM departments ORDER BY sort_order, name",
        )?;
        let rows = stmt.query_map([], map_department)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_department(&self, id: Uuid) -> Result<Option<Department>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, name, parent_id, sort_order, enabled, created_at, updated_at
             FROM departments WHERE id=?1",
            params![id.to_string()],
            map_department,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn upsert_department(&self, d: &Department) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO departments (id, name, parent_id, sort_order, enabled, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, parent_id=excluded.parent_id, sort_order=excluded.sort_order,
               enabled=excluded.enabled, updated_at=excluded.updated_at",
            params![
                d.id.to_string(),
                d.name,
                d.parent_id.map(|x| x.to_string()),
                d.sort_order,
                d.enabled as i64,
                fmt_dt(d.created_at),
                fmt_dt(d.updated_at),
            ],
        )?;
        Ok(())
    }

    pub fn delete_department(&self, id: Uuid) -> Result<bool> {
        let conn = self.lock()?;
        let children: i64 = conn.query_row(
            "SELECT COUNT(*) FROM departments WHERE parent_id=?1",
            params![id.to_string()],
            |r| r.get(0),
        )?;
        if children > 0 {
            return Err(anyhow!("部门下仍有子部门，无法删除"));
        }
        let users: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE department_id=?1",
            params![id.to_string()],
            |r| r.get(0),
        )?;
        if users > 0 {
            return Err(anyhow!("部门下仍有用户，无法删除"));
        }
        let n = conn.execute(
            "DELETE FROM departments WHERE id=?1",
            params![id.to_string()],
        )?;
        Ok(n > 0)
    }

    pub fn list_roles(&self) -> Result<Vec<Role>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, permissions_json, is_system, created_at, updated_at
             FROM roles ORDER BY name",
        )?;
        let rows = stmt.query_map([], map_role)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_role(&self, id: Uuid) -> Result<Option<Role>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, name, description, permissions_json, is_system, created_at, updated_at
             FROM roles WHERE id=?1",
            params![id.to_string()],
            map_role,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn upsert_role(&self, r: &Role) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO roles (id, name, description, permissions_json, is_system, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, description=excluded.description,
               permissions_json=excluded.permissions_json, updated_at=excluded.updated_at",
            params![
                r.id.to_string(),
                r.name,
                r.description,
                serde_json::to_string(&r.permissions)?,
                r.is_system as i64,
                fmt_dt(r.created_at),
                fmt_dt(r.updated_at),
            ],
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
        let conn = self.lock()?;
        let n = conn.execute("DELETE FROM roles WHERE id=?1", params![id.to_string()])?;
        Ok(n > 0)
    }

    pub fn list_users(&self) -> Result<Vec<UserAccount>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, username, display_name, password_hash, department_id, role_ids_json,
                    enabled, created_at, updated_at
             FROM users ORDER BY username",
        )?;
        let rows = stmt.query_map([], map_user)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_user(&self, id: Uuid) -> Result<Option<UserAccount>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, username, display_name, password_hash, department_id, role_ids_json,
                    enabled, created_at, updated_at
             FROM users WHERE id=?1",
            params![id.to_string()],
            map_user,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_user_by_username(&self, username: &str) -> Result<Option<UserAccount>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, username, display_name, password_hash, department_id, role_ids_json,
                    enabled, created_at, updated_at
             FROM users WHERE username=?1",
            params![username],
            map_user,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn upsert_user(&self, u: &UserAccount) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO users (id, username, display_name, password_hash, department_id,
                                role_ids_json, enabled, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
             ON CONFLICT(id) DO UPDATE SET
               username=excluded.username, display_name=excluded.display_name,
               password_hash=excluded.password_hash, department_id=excluded.department_id,
               role_ids_json=excluded.role_ids_json, enabled=excluded.enabled,
               updated_at=excluded.updated_at",
            params![
                u.id.to_string(),
                u.username,
                u.display_name,
                u.password_hash,
                u.department_id.map(|x| x.to_string()),
                serde_json::to_string(
                    &u.role_ids
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                )?,
                u.enabled as i64,
                fmt_dt(u.created_at),
                fmt_dt(u.updated_at),
            ],
        )?;
        Ok(())
    }

    pub fn delete_user(&self, id: Uuid) -> Result<bool> {
        let conn = self.lock()?;
        let n = conn.execute("DELETE FROM users WHERE id=?1", params![id.to_string()])?;
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

fn map_department(row: &Row<'_>) -> rusqlite::Result<Department> {
    let parent: Option<String> = row.get(2)?;
    let created: String = row.get(5)?;
    let updated: String = row.get(6)?;
    Ok(Department {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: row.get(1)?,
        parent_id: parent
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
        sort_order: row.get(3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_role(row: &Row<'_>) -> rusqlite::Result<Role> {
    let perms_json: String = row.get(3)?;
    let created: String = row.get(5)?;
    let updated: String = row.get(6)?;
    let permissions: Vec<String> = serde_json::from_str(&perms_json).unwrap_or_default();
    Ok(Role {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: row.get(1)?,
        description: row.get(2)?,
        permissions,
        is_system: row.get::<_, i64>(4)? != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}

fn map_user(row: &Row<'_>) -> rusqlite::Result<UserAccount> {
    let dept: Option<String> = row.get(4)?;
    let roles_json: String = row.get(5)?;
    let created: String = row.get(7)?;
    let updated: String = row.get(8)?;
    let role_ids = uuid_ids_from_json(&roles_json).unwrap_or_default();
    Ok(UserAccount {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        username: row.get(1)?,
        display_name: row.get(2)?,
        password_hash: row.get(3)?,
        department_id: dept
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
        role_ids,
        enabled: row.get::<_, i64>(6)? != 0,
        created_at: parse_dt(&created).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_dt(&updated).unwrap_or_else(|_| Utc::now()),
    })
}
