use rusqlite::Connection;
use serde_json::{json, Map, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("SQLite error during migration: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("JSON parse error during migration for {context}: {source}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
    },
}

pub type MigrationResult<T> = Result<T, MigrationError>;

pub const LATEST_SCHEMA_VERSION: i32 = 202704;

pub const SCHEMA_02: &str = r#"
CREATE TABLE IF NOT EXISTS global (
    filename TEXT PRIMARY KEY,
    contents JSON NOT NULL
);
CREATE TABLE IF NOT EXISTS cameras (
    unique_name TINYTEXT PRIMARY KEY,
    config_json text NOT NULL,
    drivermode_json text NOT NULL,
    pipeline_jsons mediumtext NOT NULL,
    otherpaths_json TEXT NOT NULL DEFAULT '[]'
);
"#;

pub const SCHEMA_202702: &str = r#"
CREATE TABLE IF NOT EXISTS new_global (
    config_name TEXT PRIMARY KEY,
    contents JSON NOT NULL
);
INSERT INTO new_global (config_name, contents)
SELECT filename, contents
FROM global
WHERE EXISTS (
    SELECT 1 FROM sqlite_master WHERE type='table' AND name='global'
);
DROP TABLE IF EXISTS global;
ALTER TABLE new_global RENAME TO global;

CREATE TABLE IF NOT EXISTS new_cameras (
    unique_name TEXT PRIMARY KEY,
    contents JSON NOT NULL
);
INSERT INTO new_cameras (unique_name, contents)
SELECT unique_name, config_json
FROM cameras
WHERE EXISTS (
    SELECT 1 FROM sqlite_master WHERE type='table' AND name='cameras'
);
DROP TABLE IF EXISTS cameras;
ALTER TABLE new_cameras RENAME TO cameras;
"#;

/// Executes database schema migrations matching `org.photonvision.common.configuration.migrations.DbMigration`.
pub struct MigrationManager;

impl MigrationManager {
    pub fn get_user_version(conn: &Connection) -> MigrationResult<i32> {
        let ver: i32 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
        Ok(ver)
    }

    pub fn set_user_version(conn: &Connection, ver: i32) -> MigrationResult<()> {
        conn.execute_batch(&format!("PRAGMA user_version = {ver};"))?;
        Ok(())
    }

    pub fn migrate_to_latest(conn: &mut Connection) -> MigrationResult<i32> {
        let mut ver = Self::get_user_version(conn)?;

        if ver < 2 {
            conn.execute_batch(SCHEMA_02)?;
            Self::set_user_version(conn, 2)?;
            ver = 2;
        }

        if ver < 202701 {
            Self::step_202701_update_camera_config(conn)?;
            Self::set_user_version(conn, 202701)?;
            ver = 202701;
        }

        if ver < 202702 {
            conn.execute_batch(SCHEMA_202702)?;
            Self::set_user_version(conn, 202702)?;
            ver = 202702;
        }

        if ver < 202703 {
            Self::step_202703_update_status_leds(conn)?;
            Self::set_user_version(conn, 202703)?;
            ver = 202703;
        }

        if ver < 202704 {
            Self::step_202704_update_atfl(conn)?;
            Self::set_user_version(conn, 202704)?;
            ver = 202704;
        }

        Ok(ver)
    }

    fn step_202701_update_camera_config(conn: &Connection) -> MigrationResult<()> {
        // Check if legacy columns still exist on cameras table
        let mut stmt = conn.prepare(
            "SELECT unique_name, config_json, drivermode_json, pipeline_jsons, otherpaths_json FROM cameras;",
        )?;
        let rows: Vec<(String, String, String, String, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        for (unique_name, config_str, drivermode_str, pipelines_str, otherpaths_str) in rows {
            let mut config_val: Value =
                serde_json::from_str(&config_str).map_err(|source| MigrationError::Json {
                    context: unique_name.clone(),
                    source,
                })?;

            if let Some(config_obj) = config_val.as_object_mut() {
                // 1. Migrate legacy PVCameraInfo type-wrapper
                if let Some(matched_val) = config_obj.get_mut("matchedCameraInfo") {
                    if let Some(matched_obj) = matched_val.as_object_mut() {
                        if !matched_obj.contains_key("type") {
                            if let Some((cam_type_key, inner_val)) = matched_obj
                                .iter()
                                .next()
                                .map(|(k, v)| (k.clone(), v.clone()))
                            {
                                if let Some(mut inner_obj) = inner_val.as_object().cloned() {
                                    inner_obj.insert(
                                        "type".to_string(),
                                        Value::String(format!("PVCameraInfo.{cam_type_key}")),
                                    );
                                    *matched_val = Value::Object(inner_obj);
                                }
                            }
                        }
                    }
                }

                // 2. Migrate legacy otherPaths
                if let Ok(other_paths_val) = serde_json::from_str::<Value>(&otherpaths_str) {
                    if let Some(matched_obj) = config_obj
                        .get_mut("matchedCameraInfo")
                        .and_then(Value::as_object_mut)
                    {
                        matched_obj
                            .entry("otherPaths".to_string())
                            .or_insert(other_paths_val);
                    }
                }

                // 3. Migrate pipeline_jsons array of strings -> pipelineSettings array of objects
                if !config_obj.contains_key("pipelineSettings") {
                    if let Ok(Value::Array(raw_pipelines)) =
                        serde_json::from_str::<Value>(&pipelines_str)
                    {
                        let mut migrated_pipelines = Vec::new();
                        for item in raw_pipelines {
                            let parsed_tuple = match item {
                                Value::String(s) => serde_json::from_str::<Value>(&s).ok(),
                                Value::Array(_) => Some(item),
                                _ => None,
                            };
                            if let Some(Value::Array(arr)) = parsed_tuple {
                                if arr.len() >= 2 {
                                    let p_type =
                                        arr[0].as_str().unwrap_or("ReflectivePipelineSettings");
                                    if let Some(mut p_obj) = arr[1].as_object().cloned() {
                                        p_obj.insert(
                                            "type".to_string(),
                                            Value::String(p_type.to_string()),
                                        );
                                        if let Some(show_multi) =
                                            p_obj.remove("outputShowMultipleTargets")
                                        {
                                            let is_false = show_multi == Value::Bool(false)
                                                || show_multi.as_str() == Some("false");
                                            let max_targets = if is_false { 1 } else { 20 };
                                            p_obj.insert(
                                                "outputMaximumTargets".to_string(),
                                                json!(max_targets),
                                            );
                                        }
                                        migrated_pipelines.push(Value::Object(p_obj));
                                    }
                                }
                            }
                        }
                        config_obj.insert(
                            "pipelineSettings".to_string(),
                            Value::Array(migrated_pipelines),
                        );
                    }
                }

                // 4. Migrate drivermode_json
                if !config_obj.contains_key("driveModeSettings") {
                    if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(&drivermode_str) {
                        if arr.len() >= 2 {
                            config_obj.insert("driveModeSettings".to_string(), arr[1].clone());
                        }
                    }
                }
            }

            let new_config_json =
                serde_json::to_string(&config_val).map_err(|source| MigrationError::Json {
                    context: unique_name.clone(),
                    source,
                })?;

            conn.execute(
                "REPLACE INTO cameras (unique_name, config_json, drivermode_json, pipeline_jsons) VALUES (?1, ?2, 'null', '[]');",
                (&unique_name, &new_config_json),
            )?;
        }

        Ok(())
    }

    fn step_202703_update_status_leds(conn: &Connection) -> MigrationResult<()> {
        let hw_json_opt: Option<String> = conn
            .query_row(
                "SELECT contents FROM global WHERE config_name='hardwareConfig';",
                [],
                |row| row.get(0),
            )
            .ok();

        let Some(hw_json) = hw_json_opt else {
            return Ok(());
        };

        let mut hw_val: Value =
            serde_json::from_str(&hw_json).map_err(|source| MigrationError::Json {
                context: "hardwareConfig".to_string(),
                source,
            })?;

        if let Some(hw_obj) = hw_val.as_object_mut() {
            if hw_obj.contains_key("statusRGBPins") || hw_obj.contains_key("statusRGBActiveHigh") {
                let active_high = hw_obj
                    .remove("statusRGBActiveHigh")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let mut red_pin = -1;
                let mut green_pin = -1;
                let mut blue_pin = -1;

                if let Some(Value::Array(pins)) = hw_obj.remove("statusRGBPins") {
                    if pins.len() >= 3 {
                        red_pin = pins[0].as_i64().unwrap_or(-1);
                        green_pin = pins[1].as_i64().unwrap_or(-1);
                        blue_pin = pins[2].as_i64().unwrap_or(-1);
                    }
                }

                let mut led_cfg = Map::new();
                led_cfg.insert("type".to_string(), Value::String("RGB".to_string()));
                led_cfg.insert("activeHigh".to_string(), Value::Bool(active_high));
                led_cfg.insert("redPin".to_string(), json!(red_pin));
                led_cfg.insert("greenPin".to_string(), json!(green_pin));
                led_cfg.insert("bluePin".to_string(), json!(blue_pin));

                hw_obj.insert("statusLEDConfig".to_string(), Value::Object(led_cfg));

                let updated =
                    serde_json::to_string(&hw_val).map_err(|source| MigrationError::Json {
                        context: "hardwareConfig".to_string(),
                        source,
                    })?;
                conn.execute(
                    "REPLACE INTO global (config_name, contents) VALUES ('hardwareConfig', ?1);",
                    [&updated],
                )?;
            }
        }

        Ok(())
    }

    fn step_202704_update_atfl(conn: &Connection) -> MigrationResult<()> {
        let layout_opt: Option<String> = conn
            .query_row(
                "SELECT contents FROM global WHERE config_name='apriltagFieldLayout';",
                [],
                |row| row.get(0),
            )
            .ok();

        let Some(layout_json) = layout_opt else {
            return Ok(());
        };

        let mut layout_val: Value =
            serde_json::from_str(&layout_json).map_err(|source| MigrationError::Json {
                context: "apriltagFieldLayout".to_string(),
                source,
            })?;

        if let Some(obj) = layout_val.as_object_mut() {
            let mut modified = false;
            if let Some(field) = obj.remove("field") {
                obj.insert("field-dimensions".to_string(), field);
                modified = true;
            }
            if let Some(tags) = obj.remove("tags") {
                obj.insert("field-tags".to_string(), tags);
                modified = true;
            }
            if modified {
                let updated =
                    serde_json::to_string(&layout_val).map_err(|source| MigrationError::Json {
                        context: "fieldLayout".to_string(),
                        source,
                    })?;
                conn.execute(
                    "REPLACE INTO global (config_name, contents) VALUES ('fieldLayout', ?1);",
                    [&updated],
                )?;
                conn.execute(
                    "DELETE FROM global WHERE config_name = 'apriltagFieldLayout';",
                    [],
                )?;
            }
        }

        Ok(())
    }
}
