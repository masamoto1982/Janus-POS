//! 仕様書（docs/spec.md）の「事実」の部分を、コードとデータから生成する。
//!
//! 仕様書の `<!-- generated:名前 -->` から `<!-- /generated -->` までが生成物で、印の外は手書き。
//! 検査：`cargo test --features spec --test spec_tables`（食い違えば失敗）
//! 書き換え：`UPDATE_SPEC=1 cargo test --features spec --test spec_tables`（`npm run gen:spec`）
#![cfg(feature = "spec")]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use janus_pos_core::age_rules::AgeRules;
use janus_pos_core::catalog::{self, Category, CouponKind, Product, CATALOG, COUPONS, FF_MENU};
use janus_pos_core::parcel::{self, SIZES};
use janus_pos_core::tax::TaxSettings;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn rust_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    let mut out = vec![];
    walk(&root().join("src"), &mut out);
    out
}

/// 行 i の直前に続く `///` の doc コメントを一行にまとめる（属性の行は飛ばす）
fn doc_above(lines: &[&str], i: usize) -> String {
    let mut docs = vec![];
    let mut j = i;
    while j > 0 {
        let l = lines[j - 1].trim();
        if let Some(d) = l.strip_prefix("///") {
            docs.push(d.trim().to_string());
        } else if !l.starts_with("#[") {
            break;
        }
        j -= 1;
    }
    docs.reverse();
    docs.join("")
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|")
}

// ---- 16 章 時間と数量の定数 ----

fn eval(expr: &str) -> Option<f64> {
    let mut v = 1.0;
    for t in expr.split('*') {
        v *= t.trim().replace('_', "").parse::<f64>().ok()?;
    }
    Some(v)
}

fn human_ms(ms: f64) -> String {
    let ms = ms as u64;
    if ms % 60_000 == 0 {
        format!("{} 分", ms / 60_000)
    } else if ms % 1000 == 0 {
        format!("{} 秒", ms / 1000)
    } else {
        format!("{ms}ms")
    }
}

fn constants() -> String {
    let mut out = String::from("| 名前 | 値 | 場所 | 意味 |\n| --- | --- | --- | --- |\n");
    for path in rust_sources() {
        let src = read(&path);
        let lines: Vec<&str> = src.lines().collect();
        let file = path.strip_prefix(root()).unwrap().display().to_string();
        for (i, l) in lines.iter().enumerate() {
            let t = l.trim_start();
            let Some(rest) = t.strip_prefix("pub const ") else { continue };
            let Some((name, rest)) = rest.split_once(':') else { continue };
            let Some((ty, expr)) = rest.split_once('=') else { continue };
            let ty = ty.trim();
            if !["u8", "u32", "u64", "usize", "f32", "i32"].contains(&ty) {
                continue;
            }
            let expr = expr.trim().trim_end_matches(';');
            let value = match eval(expr) {
                Some(v) if name.ends_with("_MS") => human_ms(v),
                Some(v) if ty == "f32" => format!("{v}"),
                Some(v) => fmt_yen(v as u32),
                None => format!("`{expr}`"),
            };
            let doc = doc_above(&lines, i);
            let _ = writeln!(out, "| `{name}` | {value} | `{file}` | {} |", cell(&doc));
        }
    }
    out
}

// ---- 17 章 コマンド ----

fn snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn commands() -> String {
    let src = read(&root().join("src/store.rs"));
    let start = src.find("pub enum Command {").expect("Command がありません");
    let body = &src[start..];
    let end = body.find("\n}\n").unwrap();
    let lines: Vec<&str> = body[..end].lines().collect();
    let mut out = String::from("| コマンド | 項目 | 説明 |\n| --- | --- | --- |\n");
    let mut i = 1;
    while i < lines.len() {
        let l = lines[i];
        let t = l.trim();
        let variant_line = l.starts_with("    ") && !l.starts_with("     ");
        if variant_line && t.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
            let name = t.trim_end_matches([',', '{', ' ']).to_string();
            let doc = doc_above(&lines, i);
            let mut fields = vec![];
            if t.ends_with('{') {
                i += 1;
                while i < lines.len() && lines[i].trim() != "}," && lines[i].trim() != "}" {
                    let f = lines[i].trim();
                    if !f.starts_with("#[") && !f.starts_with("//") {
                        if let Some((n, _)) = f.split_once(':') {
                            fields.push(format!("`{n}`"));
                        }
                    }
                    i += 1;
                }
            }
            let _ = writeln!(out, "| `{}` | {} | {} |", snake(&name), fields.join("・"), cell(&doc));
        }
        i += 1;
    }
    out
}

// ---- 3.2 商品・FF・クーポン ----

fn shelf(p: &Product) -> &'static str {
    match p.category {
        Category::Bag => "オプション",
        Category::Service => "サービス",
        _ if p.fresh => "日配食品",
        _ if p.dates.is_some() => "加工食品",
        _ => "非食品",
    }
}

fn span(min: i32, fresh: bool) -> String {
    let (h, d) = (60, 24 * 60);
    let (n, unit) = if fresh && min.abs() < d { (min / h, "時間") } else { (min / d, "日") };
    if n < 0 {
        format!("**{} {unit}前（期限切れ）**", -n)
    } else {
        format!("{n} {unit}後")
    }
}

fn notes(p: &Product) -> String {
    let mut n = vec![];
    if p.utility_bill {
        n.push(if p.stamp_exempt { "払込票（税金、収入印紙は要らない）" } else { "払込票" }.to_string());
    }
    if p.cash_only {
        n.push("現金のみ".into());
    }
    if p.tickets > 0 {
        n.push(format!("申込券 1 枚でチケット {} 枚を発券", p.tickets));
    }
    if p.parcel_send {
        n.push(if p.parcel_collect { "荷物を送る（着払い、レジでは 0 円）" } else if p.parcel_multi { "荷物を送る（複数口）" } else { "荷物を送る（料金は受付で決まる）" }.into());
    }
    if p.parcel_pickup {
        n.push("荷物を受取る（代金引換額）".into());
    }
    if p.fresh {
        n.push("数量を増やせない".into());
    }
    n.join("・")
}

fn products() -> String {
    let tax = TaxSettings::default();
    let ages = AgeRules::default();
    let mut out = String::from("| 棚 | 商品 | コード | 価格 | 分類 | 税区分の初期値 | 年確 | 個数制限 | 販売期限 / 賞味期限 | 備考 |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for p in CATALOG.iter().filter(|p| p.category != Category::FastFood) {
        let dates = p
            .dates
            .map(|d| format!("{} / {}", span(d.sell_by_min, p.fresh), span(d.best_before_min, p.fresh)))
            .unwrap_or_default();
        let age = ages.age_for(p.code, p.category).map(|a| format!("{a}")).unwrap_or_default();
        let limit = ages.limit_for(p.code).map(|n| format!("{n} 点")).unwrap_or_default();
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            shelf(p),
            p.name,
            p.code,
            fmt_yen(p.price),
            p.category.label(),
            tax.class_for(p.code).name,
            age,
            limit,
            dates,
            notes(p)
        );
    }
    out
}

fn fmt_yen(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn ff() -> String {
    let stock: BTreeMap<&str, u32> = catalog::INITIAL_FF_STOCK.iter().copied().collect();
    let mut tabs: Vec<(&'static str, Vec<String>)> = vec![];
    for (code, tab) in FF_MENU {
        let p = catalog::find(code).expect("FF がありません");
        let s = if catalog::is_made_to_order(code) {
            "作り置きなし".to_string()
        } else {
            format!("在庫 {}", stock.get(code).copied().unwrap_or(0))
        };
        let item = format!("{} {}・{s}", p.name, p.price);
        match tabs.iter_mut().find(|(l, _)| *l == tab.label()) {
            Some((_, v)) => v.push(item),
            None => tabs.push((tab.label(), vec![item])),
        }
    }
    let mut out = String::from("| タブ | 商品（価格・初期在庫） |\n| --- | --- |\n");
    for (label, items) in tabs {
        let _ = writeln!(out, "| {label} | {} |", items.join("／"));
    }
    out
}

fn coupons() -> String {
    let mut out = String::from("| コード | 名前 | 対象 | 値引き |\n| --- | --- | --- | --- |\n");
    for c in COUPONS {
        let target = catalog::find(c.target).map_or(c.target, |p| p.name);
        let kind = match c.kind {
            CouponKind::Exchange => "対象商品 1 個の価格（引換券）".to_string(),
            CouponKind::Discount(y) => format!("{y} 円（割引券、価格が上限）"),
        };
        let _ = writeln!(out, "| `{}` | {} | {target} | {kind} |", c.code, c.name);
    }
    out
}

// ---- 10.1 宅配の料金表 ----

fn parcel_sizes() -> String {
    let mut out = String::from("| サイズ | 三辺の合計 | 重さ | 基本料金 | 地域の段階ごとの加算 |\n| --- | --- | --- | --- | --- |\n");
    let kanto = parcel::region("1000000").unwrap();
    let kyushu = parcel::region("8000000").unwrap();
    for (size, cm, g) in SIZES {
        let base = parcel::fee(size, kanto);
        let step = (parcel::fee(size, kyushu) - base) / 4;
        let _ = writeln!(out, "| {size} | {cm}cm | {}kg | {} | {step} |", g / 1000, fmt_yen(base));
    }
    out
}

fn parcel_regions() -> String {
    let mut tiers: BTreeMap<u32, Vec<(String, Vec<u32>)>> = BTreeMap::new();
    for n in 0..100u32 {
        let Ok(r) = parcel::region(&format!("{n:02}00000")) else { continue };
        let tier = (parcel::fee(60, r) - parcel::fee(60, parcel::region("1000000").unwrap())) / 110;
        let list = tiers.entry(tier).or_default();
        match list.iter_mut().find(|(l, _)| l == r.label()) {
            Some((_, v)) => v.push(n),
            None => list.push((r.label().to_string(), vec![n])),
        }
    }
    let ranges = |v: &[u32]| {
        let mut parts: Vec<String> = vec![];
        let mut i = 0;
        while i < v.len() {
            let mut j = i;
            while j + 1 < v.len() && v[j + 1] == v[j] + 1 {
                j += 1;
            }
            parts.push(if j > i { format!("{:02}〜{:02}", v[i], v[j]) } else { format!("{:02}", v[i]) });
            i = j + 1;
        }
        parts.join("・")
    };
    let mut out = String::from("| 段階 | 地域（郵便番号の上 2 桁） |\n| --- | --- |\n");
    for (tier, list) in tiers {
        let s: Vec<String> = list.iter().map(|(l, v)| format!("{l}（{}）", ranges(v))).collect();
        let _ = writeln!(out, "| {tier} | {} |", s.join("、"));
    }
    out
}

// ---- 付録 受け入れシナリオ一覧 ----

fn scenarios(spec: &str) -> String {
    let src = read(&root().join("tests/scenarios.rs"));
    let lines: Vec<&str> = src.lines().collect();
    let mut by_section: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut unassigned: Vec<String> = vec![];
    for (i, l) in lines.iter().enumerate() {
        let Some(rest) = l.strip_prefix("fn ") else { continue };
        if i == 0 || lines[i - 1].trim() != "#[test]" {
            continue;
        }
        let name = rest.split('(').next().unwrap().to_string();
        let doc = doc_above(&lines, i);
        match doc.strip_prefix("仕様 ").and_then(|d| d.split_once('：')) {
            Some((sec, what)) => by_section.entry(sec.trim().to_string()).or_default().push((name, what.to_string())),
            None => unassigned.push(name),
        }
    }
    let mut out = String::new();
    let mut seen = vec![];
    for l in spec.lines() {
        let head = l.strip_prefix("### ").or_else(|| l.strip_prefix("## "));
        let Some(head) = head else { continue };
        let Some((no, title)) = head.split_once(' ') else { continue };
        let no = no.trim_end_matches('.');
        if !no.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        let list = by_section.get(no);
        let _ = writeln!(out, "- **{no} {title}**{}", if list.is_none() { "：シナリオなし" } else { "" });
        for (name, what) in list.into_iter().flatten() {
            let _ = writeln!(out, "  - {what}（`{name}`）");
        }
        seen.push(no.to_string());
    }
    let unknown: Vec<_> = by_section.keys().filter(|k| !seen.contains(k)).collect();
    assert!(unknown.is_empty(), "仕様書にない節を指すシナリオがあります: {unknown:?}");
    let _ = writeln!(out, "\n節をまだ書いていないシナリオ（{} 件）：{}", unassigned.len(), unassigned.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join("・"));
    out
}

// ---- 書き換え・検査 ----

fn replace_block(doc: &str, name: &str, body: &str) -> String {
    let open = format!("<!-- generated:{name} -->");
    let close = "<!-- /generated -->";
    let a = doc.find(&open).unwrap_or_else(|| panic!("仕様書に {open} がありません"));
    let after = a + open.len();
    let b = after + doc[after..].find(close).unwrap_or_else(|| panic!("{open} が閉じていません"));
    format!("{}{open}\n{}{}{}", &doc[..a], body.trim_end_matches('\n').to_string() + "\n", close, &doc[b + close.len()..])
}

#[test]
fn spec_md_matches_the_code() {
    let path = root().join("../../docs/spec.md");
    let actual = read(&path);
    let mut doc = actual.clone();
    let blocks: [(&str, String); 8] = [
        ("constants", constants()),
        ("commands", commands()),
        ("products", products()),
        ("ff", ff()),
        ("coupons", coupons()),
        ("parcel-sizes", parcel_sizes()),
        ("parcel-regions", parcel_regions()),
        ("scenarios", scenarios(&actual)),
    ];
    for (name, body) in &blocks {
        doc = replace_block(&doc, name, body);
    }
    if std::env::var_os("UPDATE_SPEC").is_some() {
        std::fs::write(&path, &doc).expect("spec.md を書き込めません");
        return;
    }
    assert!(
        doc == actual,
        "docs/spec.md の生成箇所がコードと食い違っています。`cd pos && npm run gen:spec`（= `cd pos/core && UPDATE_SPEC=1 cargo test --features spec --test spec_tables`）で作り直してください"
    );
}
