//! 新型レジで報告された問題と、壁打ちで決めた仕様を、シナリオで検証する。

use janus_pos_core::authority::{Action, Mode, Phase, Side, StaffState};
use janus_pos_core::calls::CallKind;
use janus_pos_core::store::{Command, RegisterView, Store};
use serde_json::{json, Value};

const ONIGIRI: &str = "4900000000011";
const BEER: &str = "4900000000103";
const LIGHTER: &str = "4900000000134";
const LOTTERY: &str = "4900000000158";
const FIREWORKS: &str = "4900000000165";
const PARCEL: &str = "2100000000014";
const BILL: &str = "2100000000021";

fn run(s: &mut Store, v: Value) -> Result<Option<String>, String> {
    let cmd: Command = serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"));
    s.dispatch(cmd).map(|o| o.message)
}

fn ok(s: &mut Store, v: Value) {
    if let Err(e) = run(s, v.clone()) {
        panic!("{v} failed: {e}");
    }
}

fn reg(s: &Store, id: u8) -> RegisterView {
    s.snapshot()
        .registers
        .into_iter()
        .find(|r| r.id == id)
        .unwrap()
}

/// その画面でボタンが押せるか。両画面に同じボタンが並び、押せるのは操作側の画面だけ
fn allowed(v: &RegisterView, side: Side, a: Action) -> bool {
    v.active_side.is_none_or(|s| s == side)
        && v.staff
            .actions
            .iter()
            .find(|x| x.action == a)
            .unwrap()
            .allowed
}

fn scan(s: &mut Store, r: u8, side: &str, code: &str) {
    ok(
        s,
        json!({"type":"scan","register":r,"side":side,"code":code}),
    );
}

/// 名札（山田）を読んでログインし、店員画面にタッチして操作を引き継ぐ。
/// ログイン中は有人モード（ログイン直後はお客様画面が操作側）
fn login(s: &mut Store, r: u8) {
    login_as(s, r, "ST1001");
}

fn login_as(s: &mut Store, r: u8, badge: &str) {
    scan(s, r, "staff", badge);
    ok(s, json!({"type":"claim","register":r,"side":"staff"}));
}

/// 会計ボタンの後のポイントカードの最終確認（ポイントカードなしで進む）。操作側の画面で行う
fn confirm_point(s: &mut Store, r: u8) {
    let side = match reg(s, r).active_side {
        Some(Side::Customer) => "customer",
        _ => "staff",
    };
    ok(
        s,
        json!({"type":"confirm_point_card","register":r,"side":side}),
    );
}

fn advance(s: &mut Store, ms: u64) {
    let end = s.now() + ms;
    while s.now() < end {
        let t = (s.now() + 20).min(end);
        s.tick(t);
    }
}

/// 会計画面までは、どのバーコードも順番の縛りなく受け付ける。
#[test]
fn any_barcode_is_accepted_in_any_order() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", ONIGIRI);
    scan(&mut s, 1, "customer", "PC12345678");
    scan(&mut s, 1, "staff", "2000000000015");
    scan(&mut s, 1, "customer", ONIGIRI);
    let v = reg(&s, 1);
    assert_eq!(v.staff.lines.len(), 2);
    assert_eq!(v.staff.total, 160 * 2 + 3);
    assert!(v.customer.point_card);
    assert!(run(
        &mut s,
        json!({"type":"scan","register":1,"side":"staff","code":"0000"})
    )
    .is_err());
}

/// 会計画面で読んだら登録画面に戻って追加し、投入済みの現金は残る。
#[test]
fn scanning_on_payment_screen_returns_to_registering() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", ONIGIRI);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":100,"count":1}),
    );
    scan(&mut s, 1, "customer", ONIGIRI);
    let v = reg(&s, 1);
    assert_eq!(v.staff.phase, Phase::Registering);
    assert_eq!(v.staff.inserted, 100);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":500,"count":1}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.last_receipt.as_ref().unwrap().change_total, 600 - 320);
}

/// 問題7：年確は会計ボタンの時点だけ。年確が済むまで決済できない。
#[test]
fn age_check_happens_at_checkout_and_blocks_payment() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BEER);
    assert_eq!(
        reg(&s, 1).customer.age_check,
        None,
        "no age check at scan time"
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    let v = reg(&s, 1);
    assert_eq!(v.customer.age_check, Some(20));
    assert!(run(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":1000,"count":1})
    )
    .is_err());
    ok(&mut s, json!({"type":"confirm_age","register":1}));
    ok(&mut s, json!({"type":"check_id","register":1,"ok":true}));
    // 同じ取引で戻って追加しても、再確認はしない
    scan(&mut s, 1, "staff", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    assert_eq!(reg(&s, 1).customer.age_check, None);
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":1000,"count":1}),
    );
}

/// セミセルフでは年確が店員呼び出しになり、店員が来るまでお客様は確認できない。
#[test]
fn self_mode_age_check_calls_staff() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"customer"}),
    );
    confirm_point(&mut s, 2);
    assert!(s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.register == 2 && c.kind == CallKind::AgeCheck));
    assert!(!allowed(&reg(&s, 2), Side::Customer, Action::ConfirmAge));
    scan(&mut s, 2, "staff", "ST1002");
    ok(&mut s, json!({"type":"confirm_age","register":2}));
    ok(&mut s, json!({"type":"check_id","register":2,"ok":true}));
    // 名札を読んだので有人モード。モードはログアウトか 5 分の無操作でだけ戻る
    assert_eq!(reg(&s, 2).mode, Mode::Attended);
    assert_eq!(reg(&s, 2).staff.state, StaffState::Ready);
    assert!(s.snapshot().calls.is_empty());
}

/// 店舗ルール：オーナーだけが追加でき、法令ルールは外せない。
#[test]
fn only_owner_changes_age_rules() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    let add = |id: &str, pw: &str| {
        json!({"type":"add_age_rule","owner_id":id,"password":pw,
               "target":{"kind":"product","code":LOTTERY},"min_age":20})
    };
    assert!(run(&mut s, add("1001", "1111")).is_err());
    ok(&mut s, add("9001", "9999"));
    assert!(run(
        &mut s,
        json!({"type":"remove_age_rule","owner_id":"9001","password":"9999","rule_id":1})
    )
    .is_err());
    scan(&mut s, 1, "staff", LOTTERY);
    scan(&mut s, 1, "staff", LIGHTER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    assert_eq!(reg(&s, 1).customer.age_check, Some(20));
    assert_eq!(s.snapshot().age_rules.history.len(), 1);
}

/// 問題3・5：有人モードのお客様は流れを変えられず、店員はどのモードでも操作できる。
#[test]
fn customer_cannot_take_over_attended_register() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", ONIGIRI);
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    assert!(run(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"qr"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"staff"}),
    );
    assert_eq!(reg(&s, 1).staff.phase, Phase::Registering);
}

/// FF：セミセルフの FF ボタンで呼び出し → 名札でログイン（有人）して登録 → お渡し → お客様の受取確認。
#[test]
fn fast_food_flow_in_semi_self_mode() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", ONIGIRI);
    assert!(run(
        &mut s,
        json!({"type":"register_ff","register":2,"code":"FF01","qty":1})
    )
    .is_err());
    ok(&mut s, json!({"type":"ff_button","register":2}));
    login_as(&mut s, 2, "ST1001");
    ok(
        &mut s,
        json!({"type":"register_ff","register":2,"code":"FF01","qty":2}),
    );
    assert!(
        run(
            &mut s,
            json!({"type":"register_ff","register":2,"code":"FF04","qty":1})
        )
        .is_err(),
        "sold out"
    );
    // 店員が操作している間、お客様画面は操作できない（排他）
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":2,"side":"customer"})
    )
    .is_err());
    // FF のお渡し確認にチェックを入れるまで、会計に進めない
    let e = run(
        &mut s,
        json!({"type":"checkout","register":2,"side":"staff"}),
    )
    .unwrap_err();
    assert!(e.contains("お渡し確認"), "{e}");
    let ff = reg(&s, 2)
        .staff
        .lines
        .iter()
        .find(|l| l.line.code == "FF01")
        .unwrap()
        .line
        .id;
    ok(
        &mut s,
        json!({"type":"check_ff","register":2,"side":"staff","line_id":ff,"checked":true}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"staff"}),
    );
    confirm_point(&mut s, 2);
    // 会計画面ではチェックを外せない
    assert!(run(
        &mut s,
        json!({"type":"check_ff","register":2,"side":"staff","line_id":ff,"checked":false})
    )
    .is_err());
    assert_eq!(
        reg(&s, 2).mode,
        Mode::Attended,
        "badge keeps the register attended"
    );
    // 有人モードなので支払方法は店員が選ぶ
    assert!(run(
        &mut s,
        json!({"type":"select_payment","register":2,"side":"customer","method":"emoney"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"select_payment","register":2,"side":"staff","method":"emoney"}),
    );
    ok(&mut s, json!({"type":"tap_emoney","register":2}));
    assert_eq!(reg(&s, 2).last_receipt.unwrap().total, 160 + 440);
}

/// 要店員のサービス商品：お客様が読むと呼び出し、店員の受付まで決済できない。
#[test]
fn service_item_needs_staff() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", PARCEL);
    assert!(s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.kind == CallKind::Service));
    login_as(&mut s, 2, "ST1001");
    let id = reg(&s, 2).staff.lines[0].line.id;
    ok(
        &mut s,
        json!({"type":"complete_service","register":2,"line_id":id,"amount":1200}),
    );
    assert_eq!(reg(&s, 2).mode, Mode::Attended);
    assert_eq!(reg(&s, 2).customer.total, 1200);
}

/// 問題6：出金は毎回、ログインとは別の出金パスワードが要り、記録が残る。
#[test]
fn cash_out_requires_separate_password_every_time() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    let co = |pw: &str| json!({"type":"cash_out","register":1,"kind":"emergency","amount":5000,"password":pw});
    assert!(
        run(&mut s, co("1111")).is_err(),
        "login password is not the cash-out password"
    );
    ok(&mut s, co("7777"));
    ok(&mut s, co("7777"));
    let a = s.snapshot().audit;
    assert_eq!(a.len(), 3);
    assert_eq!(a[1].staff, "山田");
    // 未ログインのレジでは出金できない
    assert!(run(
        &mut s,
        json!({"type":"cash_out","register":2,"kind":"refund","amount":100,"password":"7777"})
    )
    .is_err());
}

/// 問題8：呼び出しは発生したレジだけに出て、ほかのレジには出ない。
#[test]
fn calls_stay_on_originating_register() {
    let mut s = Store::new(1, 4, 2);
    ok(&mut s, json!({"type":"call_staff","register":2}));
    ok(&mut s, json!({"type":"call_staff","register":4}));
    let snap = s.snapshot();
    assert_eq!(snap.registers[1].staff.calls.len(), 1);
    assert!(snap.registers[0].staff.calls.is_empty());
    assert!(snap.registers[2].staff.calls.is_empty());
    // 後から来たレジ4を先に片付けても、レジ2は残る
    login_as(&mut s, 4, "ST1002");
    let snap = s.snapshot();
    assert_eq!(snap.calls.len(), 1);
    assert_eq!(snap.calls[0].register, 2);
}

/// 問題10：通信ロスがあっても再送で MAT は鳴り、不通は検知される。
/// MAT は呼び鈴：ボタンに触れている間だけ鳴り、離せば止む。鳴らし直しはしない。
#[test]
fn mat_rings_despite_loss_and_reports_link_down() {
    let mut s = Store::new(7, 2, 1);
    ok(&mut s, json!({"type":"set_mat_loss","percent":60}));
    advance(&mut s, 3_000);
    ok(&mut s, json!({"type":"call_staff","register":2}));
    advance(&mut s, 8_000);
    let snap = s.snapshot();
    assert!(snap.mats[0].rings >= 1);
    assert!(snap.mats[0].buzzing, "rings while the button is held");
    assert!(snap.mat_links[0].retries > 0 || snap.mat_links[0].acked_seq == snap.bell.seq);
    ok(&mut s, json!({"type":"release_call","register":2}));
    advance(&mut s, 8_000);
    let snap = s.snapshot();
    assert!(!snap.mats[0].buzzing, "stops once released");
    // 呼び出しは開いたままでも、鳴らし直さない
    let before = snap.mats[0].rings;
    advance(&mut s, 60_000);
    assert_eq!(s.snapshot().mats[0].rings, before);
    assert!(!s.snapshot().mats[0].buzzing);
    assert!(reg(&s, 2).lamp, "the lamp stays on until staff arrives");

    ok(
        &mut s,
        json!({"type":"set_mat_offline","mat":0,"offline":true}),
    );
    advance(&mut s, 8_000);
    let snap = s.snapshot();
    assert!(!snap.mats[0].link_ok);
    assert_eq!(snap.registers[0].staff.mat_down, vec!["MAT-1".to_string()]);
}

/// お客様画面には分類や年確対象の情報を出さない（店員画面にだけ出す）。
#[test]
fn customer_view_hides_staff_only_information() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BEER);
    scan(&mut s, 1, "staff", "PC12345678");
    let v = serde_json::to_value(reg(&s, 1)).unwrap();
    let c = v["customer"].to_string();
    assert!(!c.contains("alcohol") && !c.contains(BEER) && !c.contains("12345678"));
    assert_eq!(v["staff"]["lines"][0]["age"], 20);
    assert_eq!(reg(&s, 1).mode, Mode::Attended);
}

/// 画面の操作は排他：セミセルフ（名札なし）はお客様画面、有人（名札あり）は店員画面が操作する。
#[test]
fn staff_and_customer_screens_are_exclusive() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", ONIGIRI);
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SemiSelf);
    assert_eq!(v.active_side, Some(Side::Customer));
    assert!(run(&mut s, json!({"type":"void_transaction","register":1})).is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"qr"}),
    );
    // 店員が名札を読むと有人モードになり、支払い中でも両方の画面が操作できる
    scan(&mut s, 1, "staff", "ST1002");
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::Attended);
    assert_eq!(v.active_side, None);
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"staff"}),
    );
    // 店員側が触れている一瞬は、お客様側からの操作が利かない
    assert_eq!(reg(&s, 1).active_side, Some(Side::Staff));
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"})
    )
    .is_err());
    // ログアウトするとセミセルフに戻り、お客様が続きを操作する
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SemiSelf);
    assert_eq!(v.active_side, Some(Side::Customer));
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
}

/// 年確ルールは売り場のどの商品にも個別に付けられる（オーナーのみ、法令は外せない）。
#[test]
fn owner_sets_age_rule_per_product() {
    let mut s = Store::new(1, 2, 1);
    let set = |id: &str, pw: &str, code: &str, age: Option<u8>| json!({"type":"set_product_age","owner_id":id,"password":pw,"code":code,"min_age":age});
    assert!(run(&mut s, set("1001", "1111", LOTTERY, Some(20))).is_err());
    ok(&mut s, set("9001", "9999", LOTTERY, Some(20)));
    ok(&mut s, set("9001", "9999", LIGHTER, None));
    // 酒・たばこは法令どおりで変更不可
    assert!(run(&mut s, set("9001", "9999", BEER, None)).is_err());
    assert!(run(&mut s, set("9001", "9999", BEER, Some(25))).is_err());
    // 花火も売り場の商品として個別に設定できる
    ok(&mut s, set("9001", "9999", FIREWORKS, Some(18)));
    let cat = s.snapshot().catalog;
    let age = |c: &str| cat.iter().find(|p| p.code == c).unwrap().age;
    assert_eq!(age(LOTTERY), Some(20));
    assert_eq!(age(LIGHTER), None);
    assert_eq!(age(BEER), Some(20));
    assert_eq!(age(FIREWORKS), Some(18));
}

/// 運用モードは名札で決まる。手で有人／セミセルフは選べず、セルフは機能 OFF。
#[test]
fn mode_follows_staff_badge() {
    let mut s = Store::new(1, 2, 1);
    assert!(s
        .snapshot()
        .registers
        .iter()
        .all(|r| r.mode == Mode::SemiSelf && r.staff.state == StaffState::Absent));
    login(&mut s, 1);
    assert_eq!(reg(&s, 1).mode, Mode::Attended);
    for m in ["attended", "semi_self", "full_self"] {
        assert!(run(&mut s, json!({"type":"set_mode","register":1,"mode":m})).is_err());
    }
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SemiSelf);
    assert!(run(
        &mut s,
        json!({"type":"set_mode","register":1,"mode":"full_self"})
    )
    .is_err());
}

/// セルフモード（将来の拡張）：有効にすると、お客様が FF も自分で登録できる。
#[test]
fn customer_registers_ff_in_self_mode() {
    let mut s = Store::new(1, 2, 1);
    let ff =
        |r: u8| json!({"type":"register_ff","register":r,"side":"customer","code":"FF01","qty":1});
    // セミセルフでは FF は店員の登録
    assert!(run(&mut s, ff(2)).is_err());
    s.set_self_mode_enabled(true);
    ok(
        &mut s,
        json!({"type":"set_mode","register":2,"mode":"full_self"}),
    );
    ok(&mut s, ff(2));
    let v = reg(&s, 2);
    assert_eq!(v.customer.lines.len(), 1);
    assert!(
        s.snapshot().calls.is_empty(),
        "no staff call for self-picked FF"
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"customer"}),
    );
    confirm_point(&mut s, 2);
    ok(
        &mut s,
        json!({"type":"insert_cash","register":2,"denomination":1000,"count":1}),
    );
    assert!(reg(&s, 2).customer.lines.is_empty(), "paid and settled");
}

/// 名札でログインした有人モードも、5 分間操作がなければログアウトしてセミセルフに戻る。
/// 混雑時に急に切り替わらないよう、取引中は戻さない。
#[test]
fn idle_register_returns_to_semi_self() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 2);
    advance(&mut s, 4 * 60_000);
    // 途中で操作があれば数え直す
    scan(&mut s, 2, "staff", ONIGIRI);
    ok(&mut s, json!({"type":"void_transaction","register":2}));
    advance(&mut s, 4 * 60_000);
    assert_eq!(reg(&s, 2).mode, Mode::Attended);
    advance(&mut s, 61_000);
    let v = reg(&s, 2);
    assert_eq!(v.mode, Mode::SemiSelf);
    assert_eq!(v.staff.state, StaffState::Absent);
    assert!(v.idle_revert_in.is_none());
    // 取引中は戻さない
    login(&mut s, 1);
    scan(&mut s, 1, "staff", ONIGIRI);
    advance(&mut s, 6 * 60_000);
    assert_eq!(reg(&s, 1).mode, Mode::Attended);
}

/// 店舗メニュー：お金を入れる操作はパスワード不要、抜く操作は店舗パスワードが要る。どちらも記録が残る。
#[test]
fn cash_in_needs_no_password_but_cash_out_does() {
    let mut s = Store::new(1, 2, 1);
    let cin = json!({"type":"cash_in","register":1,"kind":"deposit","amount":3000});
    // 名札でログインした有人モードでだけ使える
    assert!(run(&mut s, cin.clone()).is_err());
    login(&mut s, 1);
    let before = reg(&s, 1).staff.drawer_total;
    ok(&mut s, cin);
    ok(
        &mut s,
        json!({"type":"cash_in","register":1,"kind":"changer_refill","amount":1500}),
    );
    assert_eq!(reg(&s, 1).staff.drawer_total, before + 4500);
    let out = |pw: &str| json!({"type":"cash_out","register":1,"kind":"changer_collect","amount":4500,"password":pw});
    assert!(run(&mut s, out("")).is_err());
    ok(&mut s, out("7777"));
    assert_eq!(reg(&s, 1).staff.drawer_total, before);
    let ops: Vec<_> = s.snapshot().audit.iter().map(|a| a.operation).collect();
    assert_eq!(
        ops,
        vec!["取引外入金", "釣銭補充", "釣銭機回収", "釣銭機回収"]
    );
    // 取引中は入出金できない
    scan(&mut s, 1, "staff", ONIGIRI);
    assert!(run(
        &mut s,
        json!({"type":"cash_in","register":1,"kind":"deposit","amount":100})
    )
    .is_err());
}

/// 投入口と排出口：投入口に金額を入れて支払い、おつりは排出口に出て取り出すまで残る。
#[test]
fn inlet_pays_and_change_waits_in_outlet() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", ONIGIRI);
    // お会計前は受け付けない
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":1000})).is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    let v = reg(&s, 1);
    assert_eq!(v.outlet, 1000 - 160);
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    assert_eq!(reg(&s, 1).outlet, 0);
    assert!(run(&mut s, json!({"type":"take_outlet","register":1})).is_err());
}

/// 店舗メニューはお客様画面に一画面ずつ出す。開いている間はお客様画面側で操作し、
/// 入金・両替は投入口、出金と両替のお金は排出口を使う。
#[test]
fn store_menu_uses_inlet_and_outlet() {
    let mut s = Store::new(1, 2, 1);
    let open = |screen: &str| json!({"type":"open_menu","register":1,"screen":screen});
    assert!(
        run(&mut s, open("top")).is_err(),
        "only for logged-in staff"
    );
    login(&mut s, 1);
    ok(&mut s, open("top"));
    let v = reg(&s, 1);
    assert_eq!(v.active_side, Some(Side::Customer));
    // 取引外入金：投入口に入れた金額が入金になる
    ok(&mut s, open("deposit"));
    let before = reg(&s, 1).staff.drawer_total;
    ok(&mut s, json!({"type":"insert","register":1,"amount":2000}));
    assert_eq!(reg(&s, 1).staff.drawer_total, before + 2000);
    // 自動釣銭機：一万円を千円札に両替して排出口へ
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":"changer","exchange_to":1000}),
    );
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":1500})).is_err());
    ok(&mut s, json!({"type":"insert","register":1,"amount":10000}));
    let v = reg(&s, 1);
    assert_eq!(v.outlet_detail, vec![(1000, 10)]);
    assert_eq!(v.staff.drawer_total, before + 2000);
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    // 緊急出金：パスワードが要り、お金は排出口に出る
    ok(&mut s, open("emergency"));
    assert!(run(
        &mut s,
        json!({"type":"cash_out","register":1,"kind":"emergency","amount":5000,"password":"x"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"cash_out","register":1,"kind":"emergency","amount":5000,"password":"7777"}),
    );
    assert_eq!(reg(&s, 1).outlet, 5000);
    // 閉じると両方の画面に戻る（触れている一瞬が過ぎれば）。ログアウトでもメニューは閉じる
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":null}),
    );
    advance(&mut s, 1_000);
    assert_eq!(reg(&s, 1).active_side, None);
    ok(&mut s, open("top"));
    ok(
        &mut s,
        json!({"type":"scan","register":1,"side":"staff","code":"ST1002"}),
    );
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert!(reg(&s, 1).menu.is_none());
}

/// 有人モード：どちらも触れていなければ、FF はどちらの画面からも登録できる。
/// 触れている一瞬だけ、もう一方の画面からの操作が利かない。
#[test]
fn attended_screens_lock_only_while_touched() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "staff", "ST1001");
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::Attended);
    assert_eq!(v.active_side, None);
    assert!(allowed(&v, Side::Customer, Action::RegisterFf));
    assert!(allowed(&v, Side::Staff, Action::RegisterFf));
    // お客様画面から FF を登録（係員がお渡しする）
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"customer","code":"FF01","qty":1}),
    );
    assert!(reg(&s, 1).staff.calls.is_empty());
    // お客様画面に触れた一瞬は、店員画面から操作できない
    assert_eq!(reg(&s, 1).active_side, Some(Side::Customer));
    let e = run(&mut s, json!({"type":"claim","register":1,"side":"staff"})).unwrap_err();
    assert!(e.contains("操作中"), "{e}");
    assert!(run(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"staff","code":"FF02","qty":1})
    )
    .is_err());
    // 一瞬が過ぎれば、両方の画面に戻る
    advance(&mut s, 1_000);
    assert_eq!(reg(&s, 1).active_side, None);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"staff","code":"FF02","qty":1}),
    );
    // 2 品ともお渡し確認のチェック前で、会計に進めない
    let v = reg(&s, 1);
    assert_eq!(v.staff.lines.len(), 2);
    assert!(!allowed(&v, Side::Staff, Action::Checkout));
    assert!(allowed(&v, Side::Staff, Action::CheckFf));
    let ids: Vec<u32> = v.staff.lines.iter().map(|l| l.line.id).collect();
    let check = |id: u32, side: &str| json!({"type":"check_ff","register":1,"side":side,"line_id":id,"checked":true});
    // 店員が触れた直後は、お客様画面からはチェックできない
    assert!(run(&mut s, check(ids[0], "customer")).is_err());
    ok(&mut s, check(ids[0], "staff"));
    assert!(!allowed(&reg(&s, 1), Side::Staff, Action::Checkout));
    advance(&mut s, 1_000);
    ok(&mut s, check(ids[1], "customer"));
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
}

/// 呼び出したレジはランプで知らせる。順番待ちはなく、後から呼んだレジもすぐ点く。
/// 店員が来るまで点きっぱなしで、来れば消える。
#[test]
fn called_register_lamp_lights_at_once_until_staff_arrives() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    assert!(reg(&s, 1).lamp);
    assert!(!reg(&s, 2).lamp, "only the calling register lights");
    ok(
        &mut s,
        json!({"type":"call_staff","register":2,"topic":"tobacco"}),
    );
    assert!(
        reg(&s, 2).lamp,
        "a second call is not queued behind the first"
    );
    advance(&mut s, 30_000);
    assert!(
        reg(&s, 1).lamp && reg(&s, 2).lamp,
        "stays lit while waiting"
    );
    scan(&mut s, 1, "staff", "ST1001");
    assert!(!reg(&s, 1).lamp, "staff arrived");
    assert!(reg(&s, 2).lamp);
}

/// 財布のクレジットカードで払う。選んだ支払方法と違うカードをかざしても通らない。
#[test]
fn wallet_card_must_match_chosen_method() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", ONIGIRI);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"credit"}),
    );
    let e = run(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    )
    .unwrap_err();
    assert!(e.contains("クレジットカードが選ばれています"), "{e}");
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"credit"}),
    );
    let rc = reg(&s, 1).last_receipt.unwrap();
    assert_eq!(rc.total, 160);
    assert_eq!(rc.method, "クレジットカード");
}

/// 放置された呼び出しほど大きく鳴る。先に呼んだレジのほうが大きい。
#[test]
fn neglected_calls_ring_louder() {
    use janus_pos_core::store::{ring_volume, RING_FULL_MS, RING_MIN};
    assert_eq!(ring_volume(0), RING_MIN);
    assert_eq!(ring_volume(RING_FULL_MS * 2), 1.0);
    assert!(ring_volume(10_000) < ring_volume(30_000));
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    advance(&mut s, 20_000);
    scan(&mut s, 2, "customer", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"customer"}),
    );
    confirm_point(&mut s, 2);
    let (r1, r2) = (reg(&s, 1), reg(&s, 2));
    assert!(
        r1.ring_volume > r2.ring_volume,
        "the first caller is louder"
    );
    assert_eq!(r1.call_waiting_ms, Some(20_000));
    scan(&mut s, 1, "staff", "ST1001");
    assert_eq!(reg(&s, 1).ring_volume, 0.0);
}

/// セミセルフ画面の FF・サービス・たばこボタンは店員呼び出しのショートカット。
/// 代行収納・宅配便のバーコードを直接読んでも呼び出しになる。ランプは店員が来るまで点く。
#[test]
fn shortcut_buttons_call_staff() {
    let mut s = Store::new(1, 2, 1);
    let kinds = |s: &Store, r: u8| -> Vec<CallKind> {
        reg(s, r).staff.calls.iter().map(|c| c.kind).collect()
    };
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"tobacco"}),
    );
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"service"}),
    );
    ok(&mut s, json!({"type":"ff_button","register":1}));
    let k = kinds(&s, 1);
    assert!(
        k.contains(&CallKind::Tobacco)
            && k.contains(&CallKind::Service)
            && k.contains(&CallKind::FastFood)
    );
    assert!(reg(&s, 1).lamp);
    // 呼び出しは開いたまま（店員が来るまでランプが点く）
    ok(&mut s, json!({"type":"release_call","register":1}));
    advance(&mut s, 60_000);
    assert_eq!(kinds(&s, 1).len(), 3);
    // 宅急便のバーコードを直接読むと、サービスの呼び出しになる
    scan(&mut s, 2, "customer", PARCEL);
    assert!(kinds(&s, 2).contains(&CallKind::Service));
    // 店員が来ると、たばこ・サービスのボタン呼び出しは片付く（FF は登録するまで残る）
    login(&mut s, 1);
    assert_eq!(kinds(&s, 1), vec![CallKind::FastFood]);
    // 有人モードではボタンで呼ばない
    assert!(run(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"tobacco"})
    )
    .is_err());
}

/// 呼び出しボタンは触れている間だけ MAT を鳴らす。短く押して「押した」電文が失われても、
/// 押した回数が届いた時点で必ず鳴る（押したのに鳴らない、をなくす）。
#[test]
fn call_button_rings_only_while_held_and_every_press_rings() {
    for seed in 1..20u64 {
        let mut s = Store::new(seed, 2, 1);
        ok(&mut s, json!({"type":"set_mat_loss","percent":70}));
        advance(&mut s, 3_000);
        for (i, topic) in [json!(null), json!("tobacco"), json!("service")]
            .into_iter()
            .enumerate()
        {
            let before = s.snapshot().mats[0].rings;
            ok(
                &mut s,
                json!({"type":"call_staff","register":1,"topic":topic}),
            );
            // ちょんと押してすぐ離す
            advance(&mut s, 50);
            ok(&mut s, json!({"type":"release_call","register":1}));
            advance(&mut s, 30_000);
            let snap = s.snapshot();
            assert_eq!(
                snap.mats[0].rings,
                before + 1,
                "seed {seed} press {i}: every press rings once"
            );
            assert!(
                !snap.mats[0].buzzing,
                "seed {seed}: no re-ring after release"
            );
            assert_eq!(
                snap.mat_links[0].acked_seq, snap.bell.seq,
                "delivery is acknowledged"
            );
        }
        // FF ボタンも同じ（ショートカット）
        ok(&mut s, json!({"type":"ff_button","register":2}));
        advance(&mut s, 20_000);
        assert!(s.snapshot().mats[0].buzzing, "held FF keeps ringing");
        ok(&mut s, json!({"type":"release_call","register":2}));
        advance(&mut s, 20_000);
        assert!(!s.snapshot().mats[0].buzzing);
    }
}

/// 有人モードにしたあと（お客様画面が操作側のままでも）、ログアウトでセミセルフに戻れる。
#[test]
fn logout_returns_to_semi_self_even_while_customer_screen_holds() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "staff", "ST1001");
    assert_eq!(reg(&s, 1).mode, Mode::Attended);
    assert_eq!(reg(&s, 1).active_side, None);
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SemiSelf);
    // 取引の途中（FF 登録済み）でも戻れる
    scan(&mut s, 1, "staff", "ST1001");
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"customer","code":"FF01","qty":1}),
    );
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SemiSelf);
}

/// 有人モードでお客様画面から FF を登録し、お渡しチェックを入れて会計できる。
/// 会計ボタンの後はポイントカードの最終確認があり、それが済むと支払方法を選べる。
#[test]
fn checkout_from_held_customer_screen_then_point_card_check() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "staff", "ST1001");
    assert_eq!(reg(&s, 1).active_side, None);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"customer","code":"FF01","qty":1}),
    );
    let id = reg(&s, 1).customer.lines[0].id;
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"customer","line_id":id,"checked":true}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.customer.phase, Phase::Paying);
    assert!(v.customer.point_pending);
    // 確認が済むまで支払方法は選べない
    assert!(run(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"cash"})
    )
    .is_err());
    // 確認中にポイントカードを読めば、そのまま確認済み（登録画面には戻らない）
    scan(&mut s, 1, "customer", "PC12345678");
    let v = reg(&s, 1);
    assert_eq!(v.customer.phase, Phase::Paying);
    assert!(!v.customer.point_pending && v.customer.point_card);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"cash"}),
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":500}));
    assert_eq!(reg(&s, 1).last_receipt.unwrap().total, 220);

    // ポイントカードを先に読んでいれば、確認は済んでいる
    scan(&mut s, 1, "customer", "PC12345678");
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"customer","code":"FF03","qty":1}),
    );
    let id = reg(&s, 1).customer.lines[0].id;
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"customer","line_id":id,"checked":true}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    assert!(!reg(&s, 1).customer.point_pending);
}

/// 店員側・お客様側の区別はない。有人モードでは操作側の画面なら取引中止もロックもできる。
/// 年齢確認ボタンはお客様側の画面にだけ効く（年確待ちの間はお客様側が操作側になる）。
#[test]
fn either_screen_can_operate_except_age_confirmation() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "staff", "ST1001");
    assert_eq!(reg(&s, 1).active_side, None);
    scan(&mut s, 1, "customer", BEER);
    assert!(allowed(
        &reg(&s, 1),
        Side::Customer,
        Action::VoidTransaction
    ));
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    let v = reg(&s, 1);
    assert!(v.staff.age_pending);
    assert_eq!(v.active_side, Some(Side::Customer));
    assert!(allowed(&v, Side::Customer, Action::ConfirmAge));
    assert!(!allowed(&v, Side::Staff, Action::ConfirmAge));
    ok(&mut s, json!({"type":"confirm_age","register":1}));
    // 取引中止もお客様側の画面から（操作側なので）できる
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    ok(&mut s, json!({"type":"lock","register":1}));
    assert_eq!(reg(&s, 1).staff.state, StaffState::Locked);
}

/// 係員が画面を持ったまま会計しても、年齢確認ボタンはお客様側の画面で押せる。
/// 係員側の画面からは押せず、操作側の画面も切り替わらない。
#[test]
fn age_button_works_only_on_customer_screen_while_staff_holds() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    let v = reg(&s, 1);
    assert!(v.staff.age_pending);
    assert_eq!(v.active_side, Some(Side::Staff));
    let e = run(
        &mut s,
        json!({"type":"confirm_age","register":1,"side":"staff"}),
    )
    .unwrap_err();
    assert!(e.contains("お客様側"), "{e}");
    ok(
        &mut s,
        json!({"type":"confirm_age","register":1,"side":"customer"}),
    );
    assert!(!reg(&s, 1).staff.age_pending);
    assert_eq!(reg(&s, 1).active_side, Some(Side::Staff));
}

/// 代行収納：係員が受け付けた支払い内容・金額は、会計時にお客様が確認するまで決済できない。
/// 確認ボタンは年確と同じく、お客様側の画面からしか押せない（係員が画面を持っていても）。
/// 宅配便は確認の対象外。
#[test]
fn bill_payment_is_confirmed_by_the_customer_on_the_customer_screen() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BILL);
    scan(&mut s, 1, "staff", PARCEL);
    for (i, amount) in [(0, 8_640), (1, 1_200)] {
        let id = reg(&s, 1).staff.lines[i].line.id;
        ok(
            &mut s,
            json!({"type":"complete_service","register":1,"line_id":id,"amount":amount}),
        );
    }
    // 登録中は確認を求めない
    assert_eq!(reg(&s, 1).customer.bill_check, None);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    let v = reg(&s, 1);
    assert_eq!(v.customer.bill_check, Some(8_640), "宅配便は確認しない");
    assert_eq!(v.active_side, Some(Side::Staff));
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"emoney"}),
    );
    assert!(run(&mut s, json!({"type":"tap_emoney","register":1})).is_err());
    let e = run(
        &mut s,
        json!({"type":"confirm_bill_payment","register":1,"side":"staff"}),
    )
    .unwrap_err();
    assert!(e.contains("お客様側"), "{e}");
    ok(
        &mut s,
        json!({"type":"confirm_bill_payment","register":1,"side":"customer"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.customer.bill_check, None);
    assert_eq!(
        v.active_side,
        Some(Side::Staff),
        "確認しても操作側は変わらない"
    );
    ok(&mut s, json!({"type":"tap_emoney","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.unwrap().total, 8_640 + 1_200);
}

/// 買い物点数は 10 点まで（数量の合計）。上限を超える商品は登録されず、FF も同じ。
#[test]
fn shopping_is_limited_to_ten_items() {
    let mut s = Store::new(1, 2, 1);
    for _ in 0..9 {
        scan(&mut s, 1, "customer", ONIGIRI);
    }
    login(&mut s, 1);
    advance(&mut s, 1_000);
    let ff = |qty: u32| json!({"type":"register_ff","register":1,"side":"staff","code":"FF01","qty":qty});
    let e = run(&mut s, ff(2)).unwrap_err();
    assert!(e.contains("10点"), "{e}");
    ok(&mut s, ff(1));
    let v = reg(&s, 1);
    assert_eq!(v.staff.lines.iter().map(|l| l.line.qty).sum::<u32>(), 10);
    assert!(!allowed(&v, Side::Staff, Action::RegisterFf));
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"customer","code":ONIGIRI}),
    )
    .unwrap_err();
    assert!(e.contains("10点"), "{e}");
    // 取り消せば、また登録できる
    let id = reg(&s, 1).staff.lines[0].line.id;
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"staff","line_id":id}),
    );
    scan(&mut s, 1, "customer", ONIGIRI);
}

/// 取引中止・ロック・ログアウトは店舗メニュー（☰）から行い、済んだらメニューは閉じる。
#[test]
fn void_lock_and_logout_from_the_store_menu() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "customer", ONIGIRI);
    let top = json!({"type":"open_menu","register":1,"screen":"top"});
    ok(&mut s, top.clone());
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    let v = reg(&s, 1);
    assert!(v.menu.is_none() && v.staff.lines.is_empty());
    ok(&mut s, top.clone());
    ok(&mut s, json!({"type":"lock","register":1}));
    let v = reg(&s, 1);
    assert!(v.menu.is_none());
    assert_eq!(v.staff.state, StaffState::Locked);
    scan(&mut s, 1, "staff", "ST1001");
    ok(&mut s, top);
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SemiSelf);
}

/// 年確は二段階：お客様が「X歳以上です」を押してから、店員が身分証を確認する（はい・いいえ）。
/// 「いいえ」なら年確商品を取り消すまで決済できない。
#[test]
fn staff_checks_id_after_the_customer_declares_age() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", ONIGIRI);
    scan(&mut s, 1, "staff", BEER);
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    advance(&mut s, 1_000);
    let check = |ok: bool, side: &str| json!({"type":"check_id","register":1,"side":side,"ok":ok});
    // お客様が押すまで、店員の身分証確認は押せない
    let e = run(&mut s, check(true, "staff")).unwrap_err();
    assert!(e.contains("お客様"), "{e}");
    assert!(!reg(&s, 1).staff.id_pending);
    ok(&mut s, json!({"type":"confirm_age","register":1}));
    let v = reg(&s, 1);
    assert!(v.staff.id_pending);
    assert_eq!(v.customer.age_check, None);
    // お客様側の画面からは押せない
    assert!(run(&mut s, check(true, "customer")).is_err());
    let cash = json!({"type":"insert_cash","register":1,"denomination":1000,"count":1});
    assert!(run(&mut s, cash.clone()).is_err());
    ok(&mut s, check(false, "staff"));
    let v = reg(&s, 1);
    assert!(v.staff.id_refused && !v.staff.id_pending);
    assert!(run(&mut s, cash.clone()).unwrap_err().contains("身分証"));
    // 年確商品を取り消せば、残りは会計できる
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"staff"}),
    );
    let beer = reg(&s, 1)
        .staff
        .lines
        .iter()
        .find(|l| l.line.code == BEER)
        .unwrap()
        .line
        .id;
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"staff","line_id":beer}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    assert!(!reg(&s, 1).staff.id_refused);
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"cash"}),
    );
    ok(&mut s, cash);
}
