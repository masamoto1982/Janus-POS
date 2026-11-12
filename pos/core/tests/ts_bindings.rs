#![cfg(feature = "ts")]

use std::any::TypeId;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use janus_pos_core::{Command, Snapshot};
use ts_rs::{Config, TypeVisitor, TS};

const HEADER: &str = "// 生成物：手で編集しない（pos/core の型から `npm run gen:types` で作り直す）\n";

struct Collect<'a> {
    cfg: &'a Config,
    seen: HashSet<TypeId>,
    decls: BTreeMap<String, String>,
}

impl TypeVisitor for Collect<'_> {
    fn visit<T: TS + 'static + ?Sized>(&mut self) {
        if T::output_path().is_none() || !self.seen.insert(TypeId::of::<T>()) {
            return;
        }
        let decl = T::decl(self.cfg);
        let doc = T::docs().unwrap_or_default();
        self.decls
            .insert(T::ident(self.cfg), format!("{doc}export {decl}\n"));
        T::visit_dependencies(self);
    }
}

fn render() -> String {
    let cfg = Config::new().with_large_int("number");
    let mut c = Collect {
        cfg: &cfg,
        seen: HashSet::new(),
        decls: BTreeMap::new(),
    };
    c.visit::<Snapshot>();
    c.visit::<Command>();
    let mut out = String::from(HEADER);
    for decl in c.decls.values() {
        out.push('\n');
        out.push_str(decl);
    }
    out
}

#[test]
fn types_ts_is_up_to_date() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/src/types.ts");
    let expected = render();
    if std::env::var_os("UPDATE_TYPES").is_some() {
        std::fs::write(&path, &expected).expect("types.ts を書き込めません");
        return;
    }
    let actual = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        actual == expected,
        "pos/web/src/types.ts が Rust の型と食い違っています。`npm run gen:types` で作り直してください"
    );
}
