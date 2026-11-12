pub mod age_rules;
pub mod auth;
pub mod authority;
pub mod bell;
pub mod calls;
pub mod cash;
pub mod catalog;
pub mod parcel;
pub mod rng;
pub mod store;
pub mod tax;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use store::{Command, Outcome, Snapshot, Store};

pub fn dispatch_json(store: &mut Store, json: &str) -> String {
    let result = serde_json::from_str::<Command>(json)
        .map_err(|e| format!("コマンドを解釈できません: {e}"))
        .and_then(|cmd| store.dispatch(cmd));
    match result {
        Ok(o) => serde_json::json!({ "ok": true, "message": o.message }).to_string(),
        Err(e) => serde_json::json!({ "ok": false, "error": e }).to_string(),
    }
}

pub fn snapshot_json(store: &Store) -> String {
    serde_json::to_string(&store.snapshot()).expect("snapshot is always serializable")
}
