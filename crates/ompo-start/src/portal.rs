#![forbid(unsafe_code)]

//! Pure portal-row helpers. The umbrella command supplies live source data.

use crate::sha256_hex;

pub const SCHEMA_ID: &str = "ompo:portal:v1";

/// Hash an object after removing its self-referential data_hash field.
#[must_use]
pub fn data_hash_without_self(row: &serde_json::Value) -> String {
    let mut payload = row.clone();
    if let Some(object) = payload.as_object_mut() {
        object.remove("data_hash");
    }
    let bytes = serde_json::to_vec(&payload).expect("portal row is JSON-serializable");
    sha256_hex(&bytes)
}

/// Add the content hash after the row has been assembled.
pub fn seal(mut row: serde_json::Value) -> Result<serde_json::Value, String> {
    if !row.is_object() {
        return Err("L5_PORTAL_ROW_NOT_OBJECT".to_owned());
    }
    row.as_object_mut()
        .expect("object checked")
        .remove("data_hash");
    let hash = data_hash_without_self(&row);
    row.as_object_mut()
        .expect("object checked")
        .insert("data_hash".to_owned(), serde_json::Value::String(hash));
    Ok(row)
}
