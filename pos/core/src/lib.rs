//! Janus POS — コンビニ向けセミセルフ POS シミュレーターのコア。
//!
//! UI から独立した決定的な状態機械として実装し、ネイティブの `cargo test`
//! で業務ルールを検証できるようにしている。ブラウザからは `wasm` feature の
//! [`wasm::PosSimulator`] を通して JSON でやり取りする。

pub mod age_rules;
pub mod auth;
pub mod authority;
pub mod calls;
pub mod cash;
pub mod catalog;
pub mod mat;
pub mod rng;
pub mod store;
pub mod tax;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use store::{Command, Outcome, Snapshot, Store};

/// JSON 文字列のコマンドを処理し、結果を JSON 文字列で返す。
/// `{"ok":true,"message":...}` または `{"ok":false,"error":"..."}`。
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
