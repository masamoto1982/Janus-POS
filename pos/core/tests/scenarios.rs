use janus_pos_core::authority::{Action, Mode, PaymentMethod, Phase, Side, StaffState};
use janus_pos_core::calls::CallKind;
use janus_pos_core::store::{
    Command, FfState, LineKind, RegisterView, Store, EMERGENCY_LOG_MS, EMERGENCY_PRESSES,
    EMERGENCY_WINDOW_MS,
};
use serde_json::{json, Value};

const BREAD: &str = "4900000000011";
const BEER: &str = "4900000000103";
const LIGHTER: &str = "4900000000134";
const SANDWICH: &str = "4900000000028";
const TEA: &str = "4900000000035";
const PARCEL: &str = "2100000000014";
const PARCEL_COLLECT: &str = "2100000000045";
const PARCEL_MULTI: &str = "2100000000081";
const RECEIVE: &str = "2100000000038";
const BILL: &str = "2100000000021";
const WATER_BILL: &str = "2100000000043";
const TAX_BILL: &str = "2100000000067";
const STAMP: &str = "4900000000141";
const TOBACCO: &str = "4900000000127";
const SNACK: &str = "4900000000073";
const ONIGIRI: &str = "4900000000080";

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

fn allowed(v: &RegisterView, side: Side, a: Action) -> bool {
    v.active_side.is_none_or(|s| s == side)
        && v.counter
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

fn login(s: &mut Store, r: u8) {
    login_as(s, r, "ST001");
}

fn open_staff_menu(s: &mut Store, r: u8) {
    ok(
        s,
        json!({"type":"open_menu","register":r,"side":"counter","screen":"top"}),
    );
}

fn login_as(s: &mut Store, r: u8, badge: &str) {
    scan(s, r, "counter", badge);
    ok(s, json!({"type":"claim","register":r,"side":"counter"}));
}

fn no_id_check(s: &mut Store, r: u8) {
    ok(
        s,
        json!({"type":"choose_age_check","register":r,"id_check":false}),
    );
}

fn confirm_point(s: &mut Store, r: u8) {
    let side = match reg(s, r).active_side {
        Some(Side::Floor) => "floor",
        _ => "counter",
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

/// 仕様 3.3：どのバーコードもどの順でも読める
#[test]
fn any_barcode_is_accepted_in_any_order() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    scan(&mut s, 1, "floor", "PC12345678");
    scan(&mut s, 1, "counter", "2000000000015");
    scan(&mut s, 1, "floor", BREAD);
    let v = reg(&s, 1);
    assert_eq!(v.counter.lines.len(), 3);
    assert_eq!(v.counter.total, 160 * 2 + 3);
    assert!(v.floor.point_card);
    assert!(run(
        &mut s,
        json!({"type":"scan","register":1,"side":"counter","code":"0000"})
    )
    .is_err());
}

/// 仕様 6.1：会計画面で読むと登録画面に戻って追加する
#[test]
fn scanning_on_payment_screen_returns_to_registering() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":100,"count":1}),
    );
    scan(&mut s, 1, "floor", BREAD);
    let v = reg(&s, 1);
    assert_eq!(v.counter.phase, Phase::Registering);
    assert_eq!(v.counter.inserted, 100);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert!(!reg(&s, 1).counter.point_pending);
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":500,"count":1}),
    );
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.last_receipt.as_ref().unwrap().change_total, 600 - 320);
}

/// 仕様 9：年齢確認は会計で行い、済むまで決済できない
#[test]
fn age_check_happens_at_checkout_and_blocks_payment() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BEER);
    assert_eq!(
        reg(&s, 1).counter.required_age,
        None,
        "no age check at scan time"
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    let v = reg(&s, 1);
    assert!(v.counter.age_choice_pending);
    assert!(!v.counter.point_pending);
    assert!(run(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":1000,"count":1})
    )
    .is_err());
    no_id_check(&mut s, 1);
    assert!(!reg(&s, 1).counter.age_choice_pending);
    confirm_point(&mut s, 1);
    scan(&mut s, 1, "counter", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert!(
        !reg(&s, 1).counter.point_pending,
        "point card asked once per txn"
    );
    assert!(!reg(&s, 1).counter.age_choice_pending);
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":1000,"count":1}),
    );
}

/// 仕様 9：セルフ操作モードの年齢確認はスタッフを呼ぶ
#[test]
fn self_operated_age_check_calls_staff() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"floor"}),
    );
    assert!(s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.register == 2 && c.kind == CallKind::AgeCheck));
    assert!(!allowed(&reg(&s, 2), Side::Floor, Action::ChooseAgeCheck));
    assert_eq!(reg(&s, 2).floor.age_wait, Some(20));
    scan(&mut s, 2, "counter", "ST002");
    assert_eq!(reg(&s, 2).floor.age_wait, None);
    no_id_check(&mut s, 2);
    assert_eq!(reg(&s, 2).mode, Mode::StaffOperated);
    assert_eq!(reg(&s, 2).counter.state, StaffState::Ready);
    assert!(s.snapshot().calls.is_empty());
}

/// 仕様 11.1：年確の呼び出し中も FF ボタンで呼べる
#[test]
fn ff_button_works_while_age_check_call_is_open() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"floor"}),
    );
    let v = reg(&s, 2);
    assert!(allowed(&v, Side::Floor, Action::FfButton));
    assert!(allowed(&v, Side::Floor, Action::CallStaff));
    ok(&mut s, json!({"type":"ff_button","register":2}));
    let calls = s.snapshot().calls;
    assert!(calls
        .iter()
        .any(|c| c.register == 2 && c.kind == CallKind::AgeCheck));
    assert!(calls
        .iter()
        .any(|c| c.register == 2 && c.kind == CallKind::FastFood));
}

/// 仕様 14.1：年齢制限を変えられるのはオーナーだけ
#[test]
fn only_owner_changes_age_rules() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    let add = |id: &str, pw: &str| {
        json!({"type":"add_age_rule","owner_id":id,"password":pw,
               "target":{"kind":"product","code":SANDWICH},"min_age":20})
    };
    assert!(run(&mut s, add("001", "1111")).is_err());
    ok(&mut s, add("000", "9999"));
    assert!(run(
        &mut s,
        json!({"type":"remove_age_rule","owner_id":"000","password":"9999","rule_id":1})
    )
    .is_err());
    scan(&mut s, 1, "counter", SANDWICH);
    scan(&mut s, 1, "counter", LIGHTER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert_eq!(reg(&s, 1).counter.required_age, Some(20));
    assert_eq!(s.snapshot().age_rules.history.len(), 1);
}

/// 仕様 4.2：スタッフ操作モードのレジを売り場側から乗っ取れない
#[test]
fn customer_cannot_take_over_staff_operated_register() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    assert!(run(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"floor","method":"qr"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"counter"}),
    );
    assert_eq!(reg(&s, 1).counter.phase, Phase::Registering);
}

/// 仕様 8：セルフ操作モードの FF は呼び出しからスタッフが登録する
#[test]
fn fast_food_flow_in_self_operated_mode() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", BREAD);
    assert!(run(
        &mut s,
        json!({"type":"register_ff","register":2,"code":"FF01","qty":1})
    )
    .is_err());
    ok(&mut s, json!({"type":"ff_button","register":2}));
    login_as(&mut s, 2, "ST001");
    ok(
        &mut s,
        json!({"type":"register_ff","register":2,"code":"FF01","qty":2}),
    );
    assert!(
        run(
            &mut s,
            json!({"type":"register_ff","register":2,"code":"FF23","qty":1})
        )
        .is_err(),
        "sold out"
    );
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":2,"side":"floor"})
    )
    .is_err());
    let e = run(
        &mut s,
        json!({"type":"checkout","register":2,"side":"counter"}),
    )
    .unwrap_err();
    assert!(e.contains("お渡し確認"), "{e}");
    let ff = reg(&s, 2)
        .counter
        .lines
        .iter()
        .find(|l| l.line.code == "FF01")
        .unwrap()
        .line
        .id;
    ok(
        &mut s,
        json!({"type":"check_ff","register":2,"side":"counter","line_id":ff,"handed":1}),
    );
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":2,"side":"counter"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"check_ff","register":2,"side":"counter","line_id":ff,"handed":2}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"counter"}),
    );
    confirm_point(&mut s, 2);
    assert!(run(
        &mut s,
        json!({"type":"check_ff","register":2,"side":"counter","line_id":ff,"handed":2})
    )
    .is_err());
    assert_eq!(
        reg(&s, 2).mode,
        Mode::StaffOperated,
        "badge keeps the register staff_operated"
    );
    assert!(run(
        &mut s,
        json!({"type":"select_payment","register":2,"side":"floor","method":"emoney"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"select_payment","register":2,"side":"counter","method":"emoney"}),
    );
    ok(&mut s, json!({"type":"tap_emoney","register":2}));
    assert_eq!(reg(&s, 2).last_receipt.unwrap().total, 160 + 440);
}

/// 仕様 10.2：荷物を照合してから引き渡す
#[test]
fn pickup_matches_the_parcel_then_hands_it_over() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", RECEIVE);
    assert!(s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.kind == CallKind::Service));
    for code in [RECEIVE, BREAD, PARCEL] {
        assert!(run(
            &mut s,
            json!({"type":"scan","register":2,"side":"floor","code":code})
        )
        .is_err());
    }
    login_as(&mut s, 2, "ST001");
    assert_eq!(reg(&s, 2).mode, Mode::StaffOperated);
    let no = reg(&s, 2).counter.lines[0]
        .line
        .pickup
        .clone()
        .expect("お預かりの番号")
        .no;
    let amount = reg(&s, 2).counter.lines[0]
        .line
        .pickup
        .clone()
        .unwrap()
        .amount;
    assert!(amount > 0);
    assert_eq!(reg(&s, 2).floor.total, amount);
    assert!(run(&mut s, json!({"type":"pickup_handover","register":2})).is_err());
    let id = reg(&s, 2).counter.lines[0].line.id;
    assert!(run(
        &mut s,
        json!({"type":"complete_service","register":2,"line_id":id,"amount":0})
    )
    .is_err());
    assert!(!allowed(&reg(&s, 2), Side::Counter, Action::Checkout));
    let other = if no.starts_with('9') {
        "PK000000000000"
    } else {
        "PK999999999999"
    };
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"counter","code":other})
    )
    .is_err());
    assert!(reg(&s, 2).counter.lines[0]
        .line
        .pickup
        .as_ref()
        .unwrap()
        .wrong
        .is_some());
    let right = format!("PK{}", no.replace('-', ""));
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"floor","code":right})
    )
    .is_err());
    scan(&mut s, 2, "counter", &right);
    let pk = reg(&s, 2).counter.lines[0].line.pickup.clone().unwrap();
    assert!(pk.matched && pk.wrong.is_none());
    let v = reg(&s, 2);
    assert!(pk.amount >= 1_000);
    assert_eq!(v.floor.total, pk.amount);
    assert!(run(&mut s, json!({"type":"pickup_handover","register":2})).is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"counter"}),
    );
    assert!(!reg(&s, 2).counter.point_pending);
    ok(
        &mut s,
        json!({"type":"select_payment","register":2,"side":"counter","method":"cash"}),
    );
    ok(
        &mut s,
        json!({"type":"insert_cash","register":2,"denomination":10000,"count":1}),
    );
    ok(&mut s, json!({"type":"finish_insert","register":2}));
    let v = reg(&s, 2);
    assert_eq!(v.last_receipt.as_ref().map(|r| r.total), Some(pk.amount));
    assert_eq!(v.pickup_handover.as_deref(), Some(pk.no.as_str()));
    ok(&mut s, json!({"type":"pickup_handover","register":2}));
    let v = reg(&s, 2);
    assert!(v.counter.lines.is_empty() && v.pickup_handover.is_none());
    assert!(!s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.kind == CallKind::Service && c.register == 2));
    scan(&mut s, 2, "counter", BREAD);
}

/// 仕様 10.2：保管場所の荷物は有り・無しで決める
#[test]
fn pickup_can_be_decided_by_found_or_missing() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", RECEIVE);
    login_as(&mut s, 2, "ST001");
    let pk = |s: &Store| reg(s, 2).counter.lines[0].line.pickup.clone().unwrap();
    ok(
        &mut s,
        json!({"type":"pickup_found","register":2,"found":false}),
    );
    assert!(pk(&s).missing && !pk(&s).matched);
    ok(
        &mut s,
        json!({"type":"pickup_found","register":2,"found":null}),
    );
    assert!(!pk(&s).missing && !pk(&s).matched);
    ok(
        &mut s,
        json!({"type":"pickup_found","register":2,"found":false}),
    );
    ok(&mut s, json!({"type":"pickup_handover","register":2}));
    assert!(reg(&s, 2).counter.lines.is_empty());
    login_as(&mut s, 2, "ST001");
    scan(&mut s, 2, "counter", RECEIVE);
    ok(
        &mut s,
        json!({"type":"pickup_found","register":2,"found":true}),
    );
    assert!(pk(&s).matched);
    for found in [json!(false), json!(null)] {
        assert!(run(
            &mut s,
            json!({"type":"pickup_found","register":2,"found":found})
        )
        .is_err());
    }
    let id = reg(&s, 2).counter.lines[0].line.id;
    assert!(run(
        &mut s,
        json!({"type":"remove_line","register":2,"side":"counter","line_id":id})
    )
    .is_err());
    assert!(pk(&s).matched);
}

fn pending_parcel(s: &Store, r: u8) -> u32 {
    reg(s, r)
        .counter
        .lines
        .iter()
        .find(|l| matches!(l.line.kind, LineKind::Service { done: false, .. }))
        .map(|l| l.line.id)
        .expect("受付中の荷物")
}

/// 仕様 10.1：着払いは 0 円、受け付けた荷物は伝票で取り消す
#[test]
fn collect_parcel_is_free_and_registered_parcel_is_cancelled_by_its_slip() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", PARCEL_COLLECT);
    login_as(&mut s, 2, "ST001");
    let id = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id,"postal":"1000001"}),
    );
    let pc = |s: &Store, i: usize| reg(s, 2).counter.lines[i].line.parcel.clone().unwrap();
    assert!(pc(&s, 0).collect && pc(&s, 0).fee == 940);
    assert_eq!(reg(&s, 2).floor.total, 0);
    assert!(
        !reg(&s, 2).counter.parcel_ask,
        "複数口でなければ伝票ごとに受付が済む"
    );
    scan(&mut s, 2, "counter", PARCEL_COLLECT);
    let id2 = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id2,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id2,"postal":"5300001"}),
    );
    assert!(pc(&s, 1).collect);
    assert_ne!(pc(&s, 1).slip, pc(&s, 0).slip);
    let slip = pc(&s, 0).slip.replace('-', "");
    assert_eq!(slip.len(), 12);
    assert!(run(
        &mut s,
        json!({"type":"remove_line","register":2,"side":"counter","line_id":id})
    )
    .is_err());
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"floor","code":format!("PS{slip}")})
    )
    .is_err());
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"counter","code":"PS000000000000"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"scan","register":2,"side":"counter","code":format!("PS{slip}")}),
    );
    let v = reg(&s, 2);
    assert_eq!(v.counter.lines.len(), 1);
    assert_eq!(v.counter.lines[0].line.id, id2);
}

/// 仕様 10.1：着払いだけなら会計で支払方法を選ばずに終わる
#[test]
fn collect_parcels_finish_on_checkout_without_choosing_a_payment() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", PARCEL_COLLECT);
    login_as(&mut s, 2, "ST001");
    let id = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id,"postal":"1000001"}),
    );
    assert!(!reg(&s, 2).counter.parcel_ask);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"counter"}),
    );
    let v = reg(&s, 2);
    assert!(v.counter.lines.is_empty(), "取引はもう終わっている");
    let rc = v.last_receipt.unwrap();
    assert_eq!((rc.total, rc.paid, rc.change_total), (0, 0, 0));
    assert_eq!(rc.method, "着払い");
}

/// 仕様 10.1：次の荷物を尋ねている間に別の伝票を読める
#[test]
fn a_parcel_for_another_address_is_scanned_even_while_asking_for_more() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", PARCEL_MULTI);
    login_as(&mut s, 2, "ST001");
    let id = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id,"postal":"1000001"}),
    );
    assert!(reg(&s, 2).counter.parcel_ask);
    scan(&mut s, 2, "counter", PARCEL_COLLECT);
    let v = reg(&s, 2);
    assert!(!v.counter.parcel_ask);
    let id2 = pending_parcel(&s, 2);
    assert_ne!(id, id2);
    let p2 = v
        .counter
        .lines
        .iter()
        .find(|l| l.line.id == id2)
        .and_then(|l| l.line.parcel.clone())
        .unwrap();
    assert_eq!(p2.postal, None, "新しく読んだ荷物の届け先は改めて入力する");
}

/// 仕様 10.1：発払いと着払いが混じれば支払方法を選ぶ
#[test]
fn prepaid_and_collect_parcels_together_still_ask_for_a_payment() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", PARCEL);
    login_as(&mut s, 2, "ST001");
    let id = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id,"postal":"1000001"}),
    );
    scan(&mut s, 2, "counter", PARCEL_COLLECT);
    let id2 = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id2,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id2,"postal":"1000001"}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"counter"}),
    );
    let v = reg(&s, 2);
    assert_eq!(v.counter.lines.len(), 2, "発払いの分を払うまで取引は続く");
    assert!(v.last_receipt.is_none());
}

/// 仕様 10.1：サイズは三辺の合計と重さの二段階で決まり、バックで一段戻る
#[test]
fn parcel_size_is_decided_in_two_steps_and_back_steps_back() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", PARCEL);
    login_as(&mut s, 2, "ST001");
    let id = pending_parcel(&s, 2);
    let parcel = |s: &Store, id: u32| {
        reg(s, 2)
            .counter
            .lines
            .iter()
            .find(|l| l.line.id == id)
            .and_then(|l| l.line.parcel.clone())
    };
    assert!(run(
        &mut s,
        json!({"type":"parcel_length","register":2,"line_id":id,"sum_cm":201})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"parcel_length","register":2,"line_id":id,"sum_cm":58}),
    );
    assert_eq!(parcel(&s, id).unwrap().sum_cm, 58);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_back","register":2,"line_id":id}),
    );
    let p = parcel(&s, id).unwrap();
    assert!(p.size.is_none() && p.sum_cm == 58);
    ok(
        &mut s,
        json!({"type":"parcel_back","register":2,"line_id":id}),
    );
    assert_eq!(parcel(&s, id).unwrap().sum_cm, 0);
    ok(
        &mut s,
        json!({"type":"parcel_back","register":2,"line_id":id}),
    );
    assert!(reg(&s, 2).counter.lines.is_empty());
    assert!(!reg(&s, 2).counter.parcel_ask);
    scan(&mut s, 2, "floor", PARCEL_MULTI);
    let id = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id,"postal":"1000001"}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_next","register":2,"more":true}),
    );
    let id2 = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_back","register":2,"line_id":id2}),
    );
    let v = reg(&s, 2);
    assert_eq!(v.counter.lines.len(), 1);
    assert!(v.counter.parcel_ask);
}

/// 仕様 10.1：料金はサイズと郵便番号で決まる
#[test]
fn parcel_fee_comes_from_size_and_postal_code() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", PARCEL);
    assert!(s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.kind == CallKind::Service));
    login_as(&mut s, 2, "ST001");
    let id = pending_parcel(&s, 2);
    assert!(run(
        &mut s,
        json!({"type":"complete_service","register":2,"line_id":id,"amount":1200})
    )
    .is_err());
    assert!(run(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id,"postal":"100-0001"})
    )
    .is_err());
    let err = run(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":201,"weight_g":3000}),
    )
    .unwrap_err();
    assert_eq!(err, "200cm を超える荷物は受け付けられません");
    let err = run(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":100,"weight_g":30_100}),
    )
    .unwrap_err();
    assert_eq!(err, "30kg を超える荷物は受け付けられません");
    assert_eq!(janus_pos_core::parcel::size_for(200, 30_000), Ok(200));
    assert_eq!(janus_pos_core::parcel::size_for(170, 3_000), Ok(180));
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id,"postal":"1000001"}),
    );
    let v = reg(&s, 2);
    assert_eq!(v.floor.total, 940);
    assert!(!v.counter.parcel_ask && !v.floor.parcel_ask);
    // お届け先の違う荷物は、その荷物の伝票を新しく読んで受け付ける
    scan(&mut s, 2, "counter", PARCEL);
    let id2 = pending_parcel(&s, 2);
    assert_ne!(id, id2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id2,"sum_cm":75,"weight_g":4000}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id2,"postal":"530-0001"}),
    );
    assert_eq!(reg(&s, 2).floor.total, 940 + 1230 + 240);
    let v = reg(&s, 2);
    assert!(!v.counter.parcel_ask);
    assert!(allowed(&v, Side::Counter, Action::Checkout));
    scan(&mut s, 2, "counter", "PC12345678");
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"counter"}),
    );
    let v = reg(&s, 2);
    assert!(!v.counter.point_pending);
    ok(
        &mut s,
        json!({"type":"select_payment","register":2,"side":"counter","method":"cash"}),
    );
    ok(
        &mut s,
        json!({"type":"insert_cash","register":2,"denomination":5000,"count":1}),
    );
    ok(&mut s, json!({"type":"finish_insert","register":2}));
    assert_eq!(reg(&s, 2).last_receipt.map(|r| r.points), Some(0));
}

/// 仕様 10.1：荷物の取引ではポイントカードを確認しない
#[test]
fn parcel_skips_point_card_check() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", PARCEL);
    login_as(&mut s, 2, "ST001");
    let id = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id,"postal":"1000001"}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"counter"}),
    );
    assert!(!reg(&s, 2).counter.point_pending);
}

/// 仕様 5.2：荷物を送るはほかの商品と一緒にできない
#[test]
fn parcel_cannot_be_mixed_with_other_goods() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", BREAD);
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"floor","code":PARCEL})
    )
    .is_err());
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", PARCEL);
    scan(&mut s, 2, "floor", PARCEL);
    assert_eq!(reg(&s, 2).floor.lines.len(), 2);
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":2,"side":"floor"})
    )
    .is_err());
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"floor","code":BREAD})
    )
    .is_err());
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"floor","code":RECEIVE})
    )
    .is_err());
}

/// 仕様 13：出金は店舗メニューの中だけ
#[test]
fn cash_out_only_inside_the_store_menu() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    let co =
        json!({"type":"cash_out","register":1,"kind":"emergency","amount":5000,"password":"7777"});
    assert!(
        run(&mut s, co.clone()).is_err(),
        "店舗メニューの外では出金できない"
    );
    open_staff_menu(&mut s, 1);
    assert!(run(
        &mut s,
        json!({"type":"cash_out","register":1,"kind":"emergency","amount":5000})
    )
    .is_err());
    assert!(run(
        &mut s,
        json!({"type":"cash_out","register":1,"kind":"emergency","amount":5000,"password":"1234"})
    )
    .is_err());
    ok(&mut s, co.clone());
    ok(&mut s, co);
    let a = s.snapshot().audit;
    assert_eq!(a.len(), 2);
    assert_eq!(a[1].staff, "スタッフ001");
    assert!(run(
        &mut s,
        json!({"type":"cash_out","register":2,"kind":"refund","amount":100,"password":"7777"})
    )
    .is_err());
}

/// 仕様 13：店舗メニューはレジカウンター側でだけ開く
#[test]
fn store_menu_opens_only_on_the_counter_side() {
    let mut s = Store::new(1, 2, 1);
    let open = json!({"type":"open_menu","register":1,"screen":"top"});
    assert!(
        run(&mut s, open.clone()).is_err(),
        "ログインしたスタッフだけ"
    );
    login(&mut s, 1);
    advance(&mut s, 1_000);
    ok(&mut s, open);
    let v = reg(&s, 1);
    assert!(v.menu.is_some());
    assert_eq!(v.active_side, Some(Side::Counter));
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":null}),
    );
    assert!(reg(&s, 1).menu.is_none());
}

/// 仕様 11.1：呼び出しは呼んだレジに留まる
#[test]
fn calls_stay_on_originating_register() {
    let mut s = Store::new(1, 4, 2);
    ok(&mut s, json!({"type":"call_staff","register":2}));
    ok(&mut s, json!({"type":"call_staff","register":4}));
    let snap = s.snapshot();
    assert_eq!(snap.registers[1].counter.calls.len(), 1);
    assert!(snap.registers[0].counter.calls.is_empty());
    assert!(snap.registers[2].counter.calls.is_empty());
    login_as(&mut s, 4, "ST002");
    let snap = s.snapshot();
    assert_eq!(snap.calls.len(), 1);
    assert_eq!(snap.calls[0].register, 2);
}

/// 仕様 11.3：通信ロスがあっても Bell は鳴り、不通を知らせる
#[test]
fn bell_rings_despite_loss_and_reports_link_down() {
    let mut s = Store::new(7, 2, 1);
    ok(&mut s, json!({"type":"set_bell_loss","percent":60}));
    advance(&mut s, 3_000);
    ok(&mut s, json!({"type":"call_staff","register":2}));
    advance(&mut s, 8_000);
    let snap = s.snapshot();
    assert!(snap.bells[0].rings >= 1);
    assert!(snap.bells[0].buzzing, "rings while the button is held");
    assert!(snap.bell_links[0].retries > 0 || snap.bell_links[0].acked_seq == snap.signal.seq);
    ok(&mut s, json!({"type":"release_call","register":2}));
    advance(&mut s, 8_000);
    let snap = s.snapshot();
    assert!(!snap.bells[0].buzzing, "stops once released");
    let before = snap.bells[0].rings;
    advance(&mut s, 60_000);
    assert!(
        s.snapshot().bells[0].rings > before,
        "rings again while the call is unanswered"
    );
    assert!(reg(&s, 2).lamp, "the lamp stays on until staff arrives");

    ok(
        &mut s,
        json!({"type":"set_bell_offline","bell":0,"offline":true}),
    );
    advance(&mut s, 8_000);
    let snap = s.snapshot();
    assert!(!snap.bells[0].link_ok);
    assert_eq!(
        snap.registers[0].counter.bell_down,
        vec!["据え付け端末".to_string()]
    );
}

/// 仕様 4.2：売り場側にはスタッフだけの情報を出さない
#[test]
fn floor_view_hides_counter_only_information() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BEER);
    scan(&mut s, 1, "counter", "PC12345678");
    let v = serde_json::to_value(reg(&s, 1)).unwrap();
    let c = v["floor"].to_string();
    assert!(!c.contains("alcohol") && !c.contains(BEER) && !c.contains("12345678"));
    assert_eq!(v["counter"]["lines"][0]["age"], 20);
    assert_eq!(reg(&s, 1).mode, Mode::StaffOperated);
}

/// 仕様 4.2：レジカウンター側と売り場側は同時には操作できない
#[test]
fn counter_and_floor_screens_are_exclusive() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BREAD);
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SelfOperated);
    assert_eq!(v.active_side, Some(Side::Floor));
    assert!(run(&mut s, json!({"type":"pause","register":1})).is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"floor","method":"qr"}),
    );
    scan(&mut s, 1, "counter", "ST002");
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::StaffOperated);
    assert_eq!(v.active_side, None);
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"counter"}),
    );
    assert_eq!(reg(&s, 1).active_side, Some(Side::Counter));
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"})
    )
    .is_err());
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SelfOperated);
    assert_eq!(v.active_side, Some(Side::Floor));
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    assert!(!reg(&s, 1).floor.point_pending);
}

/// 仕様 14.1：オーナーが商品ごとに年齢制限を決める
#[test]
fn owner_sets_age_rule_per_product() {
    let mut s = Store::new(1, 2, 1);
    let set = |id: &str, pw: &str, code: &str, age: Option<u8>| json!({"type":"set_product_age","owner_id":id,"password":pw,"code":code,"min_age":age});
    assert!(run(&mut s, set("001", "1111", SANDWICH, Some(20))).is_err());
    ok(&mut s, set("000", "9999", SANDWICH, Some(20)));
    ok(&mut s, set("000", "9999", LIGHTER, None));
    assert!(run(&mut s, set("000", "9999", BEER, None)).is_err());
    assert!(run(&mut s, set("000", "9999", BEER, Some(25))).is_err());
    ok(&mut s, set("000", "9999", TEA, Some(18)));
    let cat = s.snapshot().catalog;
    let age = |c: &str| cat.iter().find(|p| p.code == c).unwrap().age;
    assert_eq!(age(SANDWICH), Some(20));
    assert_eq!(age(LIGHTER), None);
    assert_eq!(age(BEER), Some(20));
    assert_eq!(age(TEA), Some(18));
}

/// 仕様 4.1：モードはスタッフバーコードのログインに従う
#[test]
fn mode_follows_staff_badge() {
    let mut s = Store::new(1, 2, 1);
    assert!(s
        .snapshot()
        .registers
        .iter()
        .all(|r| r.mode == Mode::SelfOperated && r.counter.state == StaffState::Absent));
    login(&mut s, 1);
    assert_eq!(reg(&s, 1).mode, Mode::StaffOperated);
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SelfOperated);
}

/// 仕様 4.1：取引中は担当スタッフを切り替えられない（ログアウトしたあとも担当のまま）
#[test]
fn staff_cannot_be_switched_during_a_transaction() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    login_as(&mut s, 1, "ST002");
    assert_eq!(
        reg(&s, 1).counter.name,
        Some("スタッフ002"),
        "取引の前なら切り替えられる"
    );
    scan(&mut s, 1, "counter", BREAD);
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"counter","code":"ST001"}),
    )
    .unwrap_err();
    assert!(e.contains("担当スタッフを切り替えられません"), "{e}");
    assert_eq!(reg(&s, 1).counter.name, Some("スタッフ002"));
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert!(run(
        &mut s,
        json!({"type":"scan","register":1,"side":"counter","code":"ST001"})
    )
    .is_err());
    login_as(&mut s, 1, "ST002");
    assert_eq!(reg(&s, 1).mode, Mode::StaffOperated);
}

/// 仕様 4.1：セルフ操作モードの取引に応答したスタッフが担当になる
#[test]
fn staff_who_answers_a_self_transaction_becomes_its_owner() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BREAD);
    ok(&mut s, json!({"type":"ff_button","register":1}));
    login_as(&mut s, 1, "ST002");
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert!(run(
        &mut s,
        json!({"type":"scan","register":1,"side":"counter","code":"ST001"})
    )
    .is_err());
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    login(&mut s, 1);
    assert_eq!(
        reg(&s, 1).counter.name,
        Some("スタッフ001"),
        "取引が終われば誰でもログインできる"
    );
}

/// 仕様 4.1：取引を終えると、後始末が済んだところで担当がログアウトしてセルフ操作モードに戻る
#[test]
fn owner_logs_out_after_each_transaction() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    pay_cash(&mut s, 1);
    let v = reg(&s, 1);
    assert_eq!(
        v.mode,
        Mode::StaffOperated,
        "お釣りを返すまでは担当がログインしたまま"
    );
    assert!(v.outlet > 0);
    assert!(
        run(
            &mut s,
            json!({"type":"scan","register":1,"side":"counter","code":"ST002"})
        )
        .is_ok(),
        "後始末だけなら次の取引のスタッフがログインできる"
    );
    login(&mut s, 1);
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::StaffOperated);
    advance(&mut s, 3_100);
    let v = reg(&s, 1);
    assert!(v.last_receipt.is_none());
    assert_eq!(
        v.mode,
        Mode::StaffOperated,
        "後始末の途中にログインし直したので、次の取引の担当になる"
    );

    scan(&mut s, 1, "counter", BREAD);
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SelfOperated, "取引中止も取引の終わり");
    assert_eq!(v.counter.state, StaffState::Absent);

    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    pay_cash(&mut s, 1);
    scan(&mut s, 1, "floor", TEA);
    let v = reg(&s, 1);
    assert_eq!(
        v.mode,
        Mode::SelfOperated,
        "後始末の途中で次の取引が始まると担当はログアウトする"
    );
    assert_eq!(v.floor.lines.len(), 1);
}

/// 仕様 4.3：セルフ操作モードではお客様が FF を登録できない
#[test]
fn customer_cannot_register_ff_in_self_operated() {
    let mut s = Store::new(1, 2, 1);
    assert!(run(
        &mut s,
        json!({"type":"register_ff","register":2,"side":"floor","code":"FF01","qty":1})
    )
    .is_err());
    ok(&mut s, json!({"type":"ff_button","register":2}));
}

/// 仕様 12.2：無操作のレジはセルフ操作モードに戻る
#[test]
fn idle_register_returns_to_self_operated() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 2);
    advance(&mut s, 4 * 60_000);
    scan(&mut s, 2, "counter", BREAD);
    ok(&mut s, json!({"type":"void_transaction","register":2}));
    assert_eq!(
        reg(&s, 2).mode,
        Mode::SelfOperated,
        "取引を終えると担当はログアウトする"
    );
    login(&mut s, 2);
    advance(&mut s, 4 * 60_000);
    assert_eq!(reg(&s, 2).mode, Mode::StaffOperated);
    advance(&mut s, 61_000);
    let v = reg(&s, 2);
    assert_eq!(v.mode, Mode::SelfOperated);
    assert_eq!(v.counter.state, StaffState::Absent);
    assert!(v.idle_revert_in.is_none());
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    ok(&mut s, json!({"type":"insert","register":1,"amount":100}));
    let v = reg(&s, 1);
    assert!(v.idle_voids);
    assert!(v.idle_revert_in.is_some());
    advance(&mut s, 4 * 60_000);
    assert_eq!(
        reg(&s, 1).mode,
        Mode::StaffOperated,
        "財布を取りに行くくらいは待つ"
    );
    advance(&mut s, 61_000);
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SelfOperated);
    assert!(v.floor.lines.is_empty());
    assert_eq!(v.floor.inserted, 0);
    assert_eq!(v.outlet, 100);
    scan(&mut s, 2, "floor", BREAD);
    assert!(reg(&s, 2).idle_voids);
    advance(&mut s, 5 * 60_000 + 1_000);
    assert!(reg(&s, 2).floor.lines.is_empty());
    assert!(reg(&s, 2).idle_revert_in.is_none());
}

/// 仕様 13：店舗メニューから入金・出金する
#[test]
fn cash_in_and_out_from_the_floor_side_menu() {
    let mut s = Store::new(1, 2, 1);
    let cin = json!({"type":"cash_in","register":1,"kind":"deposit","amount":3000});
    assert!(run(&mut s, cin.clone()).is_err());
    login(&mut s, 1);
    assert!(
        run(&mut s, cin.clone()).is_err(),
        "店舗メニューの外では入金できない"
    );
    open_staff_menu(&mut s, 1);
    let before = reg(&s, 1).counter.drawer_total;
    ok(&mut s, cin);
    ok(
        &mut s,
        json!({"type":"cash_in","register":1,"kind":"changer_refill","amount":1500}),
    );
    assert_eq!(reg(&s, 1).counter.drawer_total, before + 4500);
    ok(
        &mut s,
        json!({"type":"cash_out","register":1,"kind":"changer_collect","amount":4500,"password":"7777"}),
    );
    assert_eq!(reg(&s, 1).counter.drawer_total, before);
    let ops: Vec<_> = s.snapshot().audit.iter().map(|a| a.operation).collect();
    assert_eq!(ops, vec!["取引外入金", "釣銭補充", "釣銭機回収"]);
    scan(&mut s, 1, "counter", BREAD);
    assert!(run(
        &mut s,
        json!({"type":"cash_in","register":1,"kind":"deposit","amount":100})
    )
    .is_err());
}

/// 仕様 6.5：入金ボックスで払い、お釣りは出金ボックスで待つ
#[test]
fn inlet_pays_and_change_waits_in_outlet() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BREAD);
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":1000})).is_err());
    login(&mut s, 1);
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":1000})).is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    ok(&mut s, json!({"type":"insert","register":1,"amount":100}));
    assert!(
        run(&mut s, json!({"type":"finish_insert","register":1})).is_err(),
        "not enough yet"
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    let v = reg(&s, 1);
    assert_eq!(v.counter.inserted, 1100);
    assert_eq!(
        v.counter.method,
        Some(PaymentMethod::Cash),
        "cash picks itself"
    );
    assert!(v.last_receipt.is_none(), "waits for Enter");
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.outlet, 1100 - 160);
    assert!(v.last_receipt.is_some());
    let at = v.last_receipt.as_ref().unwrap().at;
    s.tick(at + 5000);
    assert!(reg(&s, 1).last_receipt.is_some());
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    assert_eq!(reg(&s, 1).outlet, 0);
    s.tick(at + 7999);
    assert!(
        reg(&s, 1).last_receipt.is_some(),
        "thanks shows 3 s after the change is taken"
    );
    s.tick(at + 8000);
    assert!(reg(&s, 1).last_receipt.is_none());
    assert!(run(&mut s, json!({"type":"take_outlet","register":1})).is_err());
}

/// 仕様 13：店舗メニューも入金・出金ボックスを使う
#[test]
fn store_menu_uses_inlet_and_outlet() {
    let mut s = Store::new(1, 2, 1);
    let open =
        |screen: &str| json!({"type":"open_menu","register":1,"side":"counter","screen":screen});
    assert!(
        run(&mut s, open("top")).is_err(),
        "only for logged-in staff"
    );
    login(&mut s, 1);
    open_staff_menu(&mut s, 1);
    let v = reg(&s, 1);
    assert_eq!(v.active_side, Some(Side::Counter));
    ok(&mut s, open("deposit"));
    let before = reg(&s, 1).counter.drawer_total;
    ok(&mut s, json!({"type":"insert","register":1,"amount":2000}));
    assert_eq!(reg(&s, 1).counter.drawer_total, before + 2000);
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"side":"counter","screen":"changer","exchange_to":1000}),
    );
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":1500})).is_err());
    ok(&mut s, json!({"type":"insert","register":1,"amount":10000}));
    let v = reg(&s, 1);
    assert_eq!(v.outlet_detail, vec![(1000, 10)]);
    assert_eq!(v.counter.drawer_total, before + 2000);
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    ok(&mut s, open("emergency"));
    ok(
        &mut s,
        json!({"type":"cash_out","register":1,"kind":"emergency","amount":5000,"password":"7777"}),
    );
    assert_eq!(reg(&s, 1).outlet, 5000);
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"side":"counter","screen":null}),
    );
    advance(&mut s, 1_000);
    assert_eq!(reg(&s, 1).active_side, None);
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":"top"}),
    );
    ok(
        &mut s,
        json!({"type":"scan","register":1,"side":"counter","code":"ST002"}),
    );
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert!(reg(&s, 1).menu.is_none());
}

/// 仕様 4.2：スタッフ操作モードは操作した側だけ 1 秒ロックする
#[test]
fn staff_operated_screens_lock_only_while_touched() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "counter", "ST001");
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::StaffOperated);
    assert_eq!(v.active_side, None);
    assert!(allowed(&v, Side::Floor, Action::RegisterFf));
    assert!(allowed(&v, Side::Counter, Action::RegisterFf));
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"floor","code":"FF01","qty":1}),
    );
    assert!(reg(&s, 1).counter.calls.is_empty());
    assert_eq!(reg(&s, 1).active_side, Some(Side::Floor));
    let e = run(
        &mut s,
        json!({"type":"claim","register":1,"side":"counter"}),
    )
    .unwrap_err();
    assert!(e.contains("操作中"), "{e}");
    assert!(run(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"counter","code":"FF02","qty":1})
    )
    .is_err());
    advance(&mut s, 1_000);
    assert_eq!(reg(&s, 1).active_side, None);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"counter","code":"FF02","qty":1}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.counter.lines.len(), 2);
    assert!(!allowed(&v, Side::Counter, Action::Checkout));
    assert!(allowed(&v, Side::Counter, Action::CheckFf));
    let ids: Vec<u32> = v.counter.lines.iter().map(|l| l.line.id).collect();
    let check = |id: u32, side: &str| json!({"type":"check_ff","register":1,"side":side,"line_id":id,"handed":1});
    assert!(run(&mut s, check(ids[0], "floor")).is_err());
    ok(&mut s, check(ids[0], "counter"));
    assert!(!allowed(&reg(&s, 1), Side::Counter, Action::Checkout));
    advance(&mut s, 1_000);
    assert!(run(&mut s, check(ids[1], "floor")).is_err());
    ok(&mut s, check(ids[1], "counter"));
    advance(&mut s, 1_000);
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
}

/// 仕様 11.1：呼び出しはすぐに表示され、スタッフが来るまで続く
#[test]
fn called_register_lamp_lights_at_once_until_staff_arrives() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
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
    scan(&mut s, 1, "counter", "ST001");
    assert!(!reg(&s, 1).lamp, "staff arrived");
    assert!(reg(&s, 2).lamp);
}

/// 仕様 6.4：選んだ支払方法と違うカードは受け付けない
#[test]
fn wallet_card_must_match_chosen_method() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"floor","method":"credit"}),
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

/// 仕様 11.3：放置された呼び出しは大きく鳴る
#[test]
fn neglected_calls_ring_louder() {
    use janus_pos_core::store::{ring_volume, RING_FULL_MS, RING_MIN};
    assert_eq!(ring_volume(0), RING_MIN);
    assert_eq!(ring_volume(RING_FULL_MS * 2), 1.0);
    assert!(ring_volume(10_000) < ring_volume(30_000));
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    advance(&mut s, 20_000);
    scan(&mut s, 2, "floor", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"floor"}),
    );
    let (r1, r2) = (reg(&s, 1), reg(&s, 2));
    assert!(
        r1.ring_volume > r2.ring_volume,
        "the first caller is louder"
    );
    assert_eq!(r1.call_waiting_ms, Some(20_000));
    scan(&mut s, 1, "counter", "ST001");
    assert_eq!(reg(&s, 1).ring_volume, 0.0);
}

/// 仕様 11.1：最下段の呼び出しボタンでスタッフを呼ぶ
#[test]
fn shortcut_buttons_call_staff() {
    let mut s = Store::new(1, 2, 1);
    let kinds = |s: &Store, r: u8| -> Vec<CallKind> {
        reg(s, r).counter.calls.iter().map(|c| c.kind).collect()
    };
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"tobacco"}),
    );
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"voucher"}),
    );
    ok(&mut s, json!({"type":"ff_button","register":1}));
    let k = kinds(&s, 1);
    assert!(
        k.contains(&CallKind::Tobacco)
            && k.contains(&CallKind::Voucher)
            && k.contains(&CallKind::FastFood)
    );
    assert!(reg(&s, 1).lamp);
    ok(&mut s, json!({"type":"release_call","register":1}));
    advance(&mut s, 60_000);
    assert_eq!(kinds(&s, 1).len(), 3);
    scan(&mut s, 2, "floor", PARCEL);
    assert!(kinds(&s, 2).contains(&CallKind::Service));
    login(&mut s, 1);
    assert!(kinds(&s, 1).is_empty());
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert!(kinds(&s, 1).is_empty());
    assert!(!reg(&s, 1).lamp);
    login(&mut s, 1);
    ok(&mut s, json!({"type":"call_staff","register":1}));
    advance(&mut s, 1_000);
    assert!(s.snapshot().bells[0].buzzing, "SOS rings the bell");
    assert!(kinds(&s, 1).is_empty());
}

/// 仕様 11.2：レジカウンター側のベルは応援を呼ぶだけで呼び出し画面を出さない
#[test]
fn staff_bell_calls_colleagues_without_call_screen() {
    let mut s = Store::new(1, 2, 1);
    assert_eq!(reg(&s, 1).mode, Mode::SelfOperated);
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"sos"}),
    );
    advance(&mut s, 1_000);
    assert!(s.snapshot().bells[0].buzzing, "SOS rings the bell");
    assert!(
        reg(&s, 1).counter.calls.is_empty(),
        "no call record, so no call screen"
    );
    ok(&mut s, json!({"type":"release_call","register":1}));
}

/// 仕様 11.2：呼び出しボタンは押している間だけ鳴り、押すたびに鳴る
#[test]
fn call_button_rings_only_while_held_and_every_press_rings() {
    for seed in 1..20u64 {
        let mut s = Store::new(seed, 2, 1);
        ok(&mut s, json!({"type":"set_bell_loss","percent":70}));
        advance(&mut s, 3_000);
        for (i, topic) in [json!(null), json!("tobacco"), json!("voucher")]
            .into_iter()
            .enumerate()
        {
            let before = s.snapshot().bells[0].rings;
            ok(
                &mut s,
                json!({"type":"call_staff","register":1,"topic":topic}),
            );
            advance(&mut s, 50);
            ok(&mut s, json!({"type":"release_call","register":1}));
            advance(&mut s, 20_000);
            // 呼び鈴の配達だけを見る：放置で入った非常事態モードの鳴り続けは別の試験で見る
            if reg(&s, 1).emergency {
                ok(&mut s, json!({"type":"end_emergency","register":1}));
            }
            let snap = s.snapshot();
            assert_eq!(
                snap.bells[0].rings,
                before + 1,
                "seed {seed} press {i}: every press rings once"
            );
            assert!(
                !snap.bells[0].buzzing,
                "seed {seed}: no re-ring after release"
            );
            assert_eq!(
                snap.bell_links[0].acked_seq, snap.signal.seq,
                "delivery is acknowledged"
            );
        }
        ok(&mut s, json!({"type":"ff_button","register":2}));
        advance(&mut s, 20_000);
        assert!(s.snapshot().bells[0].buzzing, "held FF keeps ringing");
        ok(&mut s, json!({"type":"release_call","register":2}));
        advance(&mut s, 20_000);
        assert!(!s.snapshot().bells[0].buzzing);
    }
}

/// 仕様 12.3：売り場側が操作中でもログアウトでセルフ操作モードに戻る
#[test]
fn logout_returns_to_self_operated_even_while_floor_screen_holds() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "counter", "ST001");
    assert_eq!(reg(&s, 1).mode, Mode::StaffOperated);
    assert_eq!(reg(&s, 1).active_side, None);
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SelfOperated);
    scan(&mut s, 1, "counter", "ST001");
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"floor","code":"FF01","qty":1}),
    );
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SelfOperated);
}

/// 仕様 6.1：売り場側から会計し、ポイントカードの確認に進む
#[test]
fn checkout_from_held_floor_screen_then_point_card_check() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "counter", "ST001");
    assert_eq!(reg(&s, 1).active_side, None);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"floor","code":"FF01","qty":1}),
    );
    let id = reg(&s, 1).floor.lines[0].id;
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"counter","line_id":id,"handed":1}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.floor.phase, Phase::Paying);
    assert!(v.floor.point_pending);
    assert!(run(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"floor","method":"cash"})
    )
    .is_err());
    scan(&mut s, 1, "floor", "PC12345678");
    let v = reg(&s, 1);
    assert_eq!(v.floor.phase, Phase::Paying);
    assert!(!v.floor.point_pending && v.floor.point_card);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"floor","method":"cash"}),
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":500}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.unwrap().total, 220);

    login(&mut s, 1);
    advance(&mut s, 1_000);
    scan(&mut s, 1, "floor", "PC12345678");
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"floor","code":"FF03","qty":1}),
    );
    let id = reg(&s, 1).floor.lines[0].id;
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"counter","line_id":id,"handed":1}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    assert!(!reg(&s, 1).floor.point_pending);
}

/// 仕様 4.2：年齢確認のほかはどちらの画面からも操作できる
#[test]
fn either_screen_can_operate_except_age_confirmation() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "counter", "ST001");
    assert_eq!(reg(&s, 1).active_side, None);
    scan(&mut s, 1, "floor", BEER);
    assert!(allowed(&reg(&s, 1), Side::Floor, Action::VoidTransaction));
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    let v = reg(&s, 1);
    assert!(v.counter.age_choice_pending);
    assert!(run(
        &mut s,
        json!({"type":"choose_age_check","register":1,"side":"floor","id_check":false})
    )
    .is_err());
    no_id_check(&mut s, 1);
    ok(&mut s, json!({"type":"void_transaction","register":1}));
}

/// 仕様 9：スタッフが年齢確認をスキップすればお客様の操作は要らない
#[test]
fn staff_skipping_the_age_check_needs_no_customer_button() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    no_id_check(&mut s, 1);
    let v = reg(&s, 1);
    assert!(!v.counter.age_choice_pending && !v.counter.id_pending);
    assert!(v.counter.point_pending);
    confirm_point(&mut s, 1);
    assert_eq!(reg(&s, 1).counter.payment_blocker, None);
}

fn check_bill(s: &mut Store, r: u8, line_id: u32, checked: bool) {
    ok(
        s,
        json!({"type":"check_bill","register":r,"side":"counter","line_id":line_id,"checked":checked}),
    );
}

fn affix_stamp(s: &mut Store, r: u8, line_id: u32, affixed: bool) {
    ok(
        s,
        json!({"type":"affix_stamp","register":r,"side":"counter","line_id":line_id,"affixed":affixed}),
    );
}

/// 仕様 10.4：払込票の印紙は登録中に確認でき、確認まで決済できない
#[test]
fn revenue_stamp_can_be_confirmed_while_registering_and_blocks_payment_until_then() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BILL);
    let id = bill_ids(&s, 1)[0];
    affix_stamp(&mut s, 1, id, true);
    affix_stamp(&mut s, 1, id, false);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    check_bill(&mut s, 1, id, true);
    assert!(reg(&s, 1)
        .counter
        .payment_blocker
        .unwrap()
        .contains("収入印紙"));
    affix_stamp(&mut s, 1, id, true);
    assert_eq!(reg(&s, 1).counter.payment_blocker, None);
    ok(&mut s, json!({"type":"insert","register":1,"amount":50000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).bill_handover, Some(1));
}

/// 仕様 10.4：同じ金額でも税金の払込票には印紙が要らない
#[test]
fn a_tax_slip_of_the_same_amount_needs_no_revenue_stamp() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", TAX_BILL);
    let v = reg(&s, 1);
    assert_eq!(v.counter.bills[0].amount, 50_000);
    assert_eq!(v.counter.bills[0].stamp, 0);
    assert!(!v.counter.stamp_pending);
    let id = bill_ids(&s, 1)[0];
    assert!(run(
        &mut s,
        json!({"type":"affix_stamp","register":1,"line_id":id,"affixed":true})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    check_bill(&mut s, 1, id, true);
    assert_eq!(reg(&s, 1).counter.payment_blocker, None);
    ok(&mut s, json!({"type":"insert","register":1,"amount":50000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).bill_handover, Some(1));
}

/// たくさんの買い物で合計 56,200円（すべて 10%、税抜 51,091円）：たばこカートン 6,000円×9、化粧品 2,200円×1
fn big_basket(s: &mut Store, side: &str) {
    for (code, qty) in [("4900000000158", 9), ("4910000000069", 1)] {
        scan(s, 1, side, code);
        let id = reg(s, 1)
            .counter
            .lines
            .iter()
            .map(|l| l.line.id)
            .max()
            .unwrap();
        if qty > 1 {
            ok(
                s,
                json!({"type":"set_qty","register":1,"side":side,"line_id":id,"qty":qty}),
            );
        }
    }
    assert_eq!(reg(s, 1).counter.total, 56_200);
}

/// 仕様 10.4：税抜 5 万円の現金のお買い物はレシートに印紙が要る
#[test]
fn a_cash_purchase_of_50000_yen_before_tax_needs_a_stamp_on_the_receipt() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    big_basket(&mut s, "counter");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    no_id_check(&mut s, 1);
    confirm_point(&mut s, 1);
    ok(&mut s, json!({"type":"insert","register":1,"amount":56200}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    let v = reg(&s, 1);
    let rs = v.receipt_stamp.unwrap();
    assert_eq!((rs.stamp, rs.base), (200, 51_091));
    advance(&mut s, 10_000);
    assert!(
        reg(&s, 1).last_receipt.is_some(),
        "印紙の確認まで取引を閉じない"
    );
    assert!(run(
        &mut s,
        json!({"type":"affix_receipt_stamp","register":1,"side":"floor"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"affix_receipt_stamp","register":1,"side":"counter"}),
    );
    assert_eq!(reg(&s, 1).receipt_stamp, None);
    advance(&mut s, 10_000);
    assert!(reg(&s, 1).last_receipt.is_none());
}

/// 仕様 10.4：セルフ操作モードで印紙が要ればスタッフを呼ぶ
#[test]
fn a_self_operated_purchase_needing_a_stamp_calls_staff() {
    let mut s = Store::new(1, 2, 1);
    // 化粧品 2,200円×25＝55,000円（税抜 50,000円）
    scan(&mut s, 1, "floor", "4910000000069");
    let id = reg(&s, 1).floor.lines[0].id;
    ok(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"floor","line_id":id,"qty":25}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"floor","method":"qr"}),
    );
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"qr"}),
    );
    assert_eq!(reg(&s, 1).receipt_stamp.map(|r| r.base), Some(50_000));
    assert!(s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.kind == CallKind::ReceiptStamp));
    assert!(run(
        &mut s,
        json!({"type":"affix_receipt_stamp","register":1,"side":"counter"})
    )
    .is_err());
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"affix_receipt_stamp","register":1,"side":"counter"}),
    );
    assert!(s.snapshot().calls.is_empty());
}

/// 仕様 10.4：税抜 5 万円未満かクレジットカードなら印紙は要らない
#[test]
fn a_receipt_under_50000_yen_before_tax_or_paid_by_credit_needs_no_stamp() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    big_basket(&mut s, "counter");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    no_id_check(&mut s, 1);
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"credit"}),
    );
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"credit"}),
    );
    assert_eq!(reg(&s, 1).receipt_stamp, None);
}

/// 仕様 10.4：セルフ操作モードの払込票の印紙はスタッフを待つ
#[test]
fn revenue_stamp_waits_for_staff_in_self_operated_mode() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BILL);
    let id = bill_ids(&s, 1)[0];
    assert!(run(
        &mut s,
        json!({"type":"affix_stamp","register":1,"side":"counter","line_id":id,"affixed":true})
    )
    .is_err());
    assert!(reg(&s, 1).floor.stamp_pending);
}

fn bill_ids(s: &Store, r: u8) -> Vec<u32> {
    reg(s, r).counter.bills.iter().map(|b| b.line_id).collect()
}

/// 仕様 10.3：払込票は会計の後にスタッフが確かめ、現金で払う
#[test]
fn utility_bills_are_checked_by_staff_after_checkout_and_paid_in_cash() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    for code in [BILL, WATER_BILL, WATER_BILL, BILL] {
        scan(&mut s, 1, "counter", code);
    }
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    let v = reg(&s, 1);
    assert!(!v.counter.point_pending && !v.counter.age_choice_pending);
    assert_eq!(v.counter.method, Some(PaymentMethod::Cash));
    assert!(v.counter.bill_unchecked);
    let rows: Vec<_> = v.counter.bills.iter().map(|b| (b.name, b.amount)).collect();
    assert_eq!(
        rows,
        [
            ("電気料金", 50_000),
            ("水道料金", 4_000),
            ("水道料金", 4_000),
            ("電気料金", 50_000)
        ]
    );
    let nos: Vec<_> = v.counter.bills.iter().map(|b| b.slip_no.clone()).collect();
    for no in &nos {
        assert_eq!(no.len(), 19, "{no}");
        assert!(no
            .split(' ')
            .all(|g| g.len() == 4 && g.bytes().all(|c| c.is_ascii_digit())));
    }
    assert_ne!(nos[0], nos[3]);
    let ids = bill_ids(&s, 1);
    assert!(v
        .counter
        .payment_blocker
        .unwrap()
        .contains("公共料金の内容"));
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":20000})).is_err());
    let e = run(
        &mut s,
        json!({"type":"check_bill","register":1,"side":"floor","line_id":ids[0],"checked":true}),
    )
    .unwrap_err();
    assert!(e.contains("レジカウンター側"), "{e}");
    check_bill(&mut s, 1, ids[0], true);
    let v = reg(&s, 1);
    assert!(v.counter.bills[0].checked);
    assert!(
        !v.counter.bills[3].checked,
        "同じ電気料金でも一枚ずつ確認する"
    );
    check_bill(&mut s, 1, ids[0], false);
    assert!(!reg(&s, 1).counter.bills[0].checked);
    for &id in &ids[..3] {
        check_bill(&mut s, 1, id, true);
    }
    assert!(reg(&s, 1).counter.bill_unchecked);
    check_bill(&mut s, 1, ids[3], true);
    let v = reg(&s, 1);
    assert!(!v.counter.bill_unchecked);
    let stamps: Vec<_> = v.counter.bills.iter().map(|b| b.stamp).collect();
    assert_eq!(
        stamps,
        [200, 0, 0, 200],
        "5万円以上の払込票だけ収入印紙が要る"
    );
    assert!(v.counter.stamp_pending);
    assert!(v.counter.payment_blocker.unwrap().contains("収入印紙"));
    assert!(run(
        &mut s,
        json!({"type":"insert","register":1,"amount":200000})
    )
    .is_err());
    let e = run(
        &mut s,
        json!({"type":"affix_stamp","register":1,"side":"floor","line_id":ids[0],"affixed":true}),
    )
    .unwrap_err();
    assert!(e.contains("レジカウンター側"), "{e}");
    assert!(
        run(
            &mut s,
            json!({"type":"affix_stamp","register":1,"line_id":ids[1],"affixed":true})
        )
        .is_err(),
        "印紙の要らない払込票は確認しない"
    );
    affix_stamp(&mut s, 1, ids[0], true);
    assert!(reg(&s, 1).counter.stamp_pending, "一枚ずつ確認する");
    affix_stamp(&mut s, 1, ids[3], true);
    let v = reg(&s, 1);
    assert!(!v.counter.stamp_pending);
    assert_eq!(v.counter.payment_blocker, None);
    for m in ["credit", "qr", "points"] {
        assert!(
            run(
                &mut s,
                json!({"type":"select_payment","register":1,"side":"counter","method":m})
            )
            .is_err(),
            "{m}"
        );
    }
    ok(
        &mut s,
        json!({"type":"insert","register":1,"amount":110000}),
    );
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    let v = reg(&s, 1);
    let r = v.last_receipt.unwrap();
    assert_eq!((r.total, r.method, r.points), (108_000, "現金", 0));
    assert_eq!(v.bill_handover, Some(4));
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    advance(&mut s, 10_000);
    assert!(reg(&s, 1).last_receipt.is_some());
    assert!(run(
        &mut s,
        json!({"type":"hand_over_bills","register":1,"side":"floor"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"hand_over_bills","register":1,"side":"counter"}),
    );
    assert_eq!(reg(&s, 1).bill_handover, None);
    advance(&mut s, 100);
    assert!(reg(&s, 1).last_receipt.is_none());
}

/// 仕様 10.3：セルフ操作モードの払込票の会計はスタッフを呼ぶ
#[test]
fn self_operated_bill_checkout_calls_staff_to_check_the_slips() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", WATER_BILL);
    scan(&mut s, 1, "floor", WATER_BILL);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    let calls = s.snapshot().calls;
    assert!(calls
        .iter()
        .any(|c| c.register == 1 && c.kind == CallKind::Bill && c.detail == "2枚"));
    let v = reg(&s, 1);
    assert!(v.floor.bill_unchecked && v.floor.staff_called);
    assert!(!v.floor.point_pending);
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":10000})).is_err());
    let id = bill_ids(&s, 1)[0];
    assert!(run(
        &mut s,
        json!({"type":"check_bill","register":1,"side":"counter","line_id":id,"checked":true})
    )
    .is_err());
    login(&mut s, 1);
    assert_eq!(reg(&s, 1).mode, Mode::StaffOperated);
    let ids = bill_ids(&s, 1);
    assert!(s.snapshot().calls.is_empty());
    check_bill(&mut s, 1, ids[0], true);
    check_bill(&mut s, 1, ids[1], true);
    assert!(s.snapshot().calls.is_empty());
    ok(&mut s, json!({"type":"insert","register":1,"amount":10000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).bill_handover, Some(2));
    scan(&mut s, 1, "counter", BREAD);
    assert_eq!(reg(&s, 1).bill_handover, None);
}

/// 仕様 5.2：払込票はほかの商品と同じ取引にできない
#[test]
fn utility_bills_cannot_share_a_transaction_with_other_goods() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"counter","code":BILL}),
    )
    .unwrap_err();
    assert!(
        e.contains("お支払い同士なら幾つでも一緒に会計できます"),
        "{e}"
    );
    ok(
        &mut s,
        json!({"type":"void_transaction","register":1,"side":"counter"}),
    );
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BILL);
    for cmd in [
        json!({"type":"scan","register":1,"side":"counter","code":BREAD}),
        json!({"type":"register_ff","register":1,"code":"FF01","qty":1}),
    ] {
        let e = run(&mut s, cmd).unwrap_err();
        assert!(
            e.contains("お支払い同士なら幾つでも一緒に会計できます"),
            "{e}"
        );
    }
    assert_eq!(reg(&s, 1).counter.lines.len(), 1);
}

/// 仕様 5.3：払込票は明細ごとに取り消せず、取引中止だけ
#[test]
fn registered_bill_slips_cannot_be_removed_only_voided() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BILL);
    scan(&mut s, 1, "floor", WATER_BILL);
    let id = reg(&s, 1).counter.lines[0].line.id;
    let try_remove = |s: &mut Store, side: &str| {
        let e = run(
            s,
            json!({"type":"remove_line","register":1,"side":side,"line_id":id}),
        )
        .unwrap_err();
        assert!(e.contains("取引中止"), "{e}");
    };
    try_remove(&mut s, "floor");
    login(&mut s, 1);
    try_remove(&mut s, "counter");
    assert_eq!(reg(&s, 1).counter.lines.len(), 2);
    ok(
        &mut s,
        json!({"type":"void_transaction","register":1,"side":"counter"}),
    );
    assert!(reg(&s, 1).counter.lines.is_empty());
}

/// 仕様 6.4：ポイント利用は付与対象の商品の分だけ
#[test]
fn points_pay_only_for_point_eligible_goods() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", "PC12345678");
    scan(&mut s, 1, "counter", TOBACCO);
    scan(&mut s, 1, "counter", SANDWICH);
    let price = |s: &Store, code: &str| {
        reg(s, 1)
            .counter
            .lines
            .iter()
            .find(|l| l.line.code == code)
            .map(|l| l.amount)
            .unwrap()
    };
    let (tob, sand) = (price(&s, TOBACCO), price(&s, SANDWICH));
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    no_id_check(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"points"}),
    );
    use_points(&mut s, 1, "all", None);
    let v = reg(&s, 1);
    assert_eq!(v.prepaid, sand);
    assert_eq!(v.counter.due, tob);
    assert!(run(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"points"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"credit"}),
    );
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"credit"}),
    );
    let r = reg(&s, 1).last_receipt.expect("決済済み");
    assert_eq!(r.method, "ポイント利用・クレジットカード");
    assert_eq!(r.points_used, sand);
    assert_eq!(
        r.points, 0,
        "no points on the part paid by points or on tobacco"
    );
    assert_eq!(r.point_balance, Some(1_250 - sand));

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", "PC12345678");
    scan(&mut s, 1, "counter", TOBACCO);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    no_id_check(&mut s, 1);
    assert!(reg(&s, 1)
        .unavailable
        .iter()
        .any(|u| u.method == PaymentMethod::Points));
}

/// 仕様 6.4：セルフ操作モードで現金を選ぶとスタッフを呼ぶ
#[test]
fn self_operated_cash_choice_calls_staff() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"floor","method":"cash"}),
    );
    assert!(reg(&s, 1).counter.calls.iter().any(|c| c.label == "現金"));
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":1000})).is_err());
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"floor","method":"emoney"}),
    );
    assert!(reg(&s, 1).counter.calls.is_empty());
}

/// 仕様 6.4：現金の案内からバックすると呼び出しを取り下げる
#[test]
fn self_operated_back_from_cash_choice_withdraws_the_call() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"floor","method":"cash"}),
    );
    assert!(reg(&s, 1).counter.calls.iter().any(|c| c.label == "現金"));
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"floor"}),
    );
    assert!(reg(&s, 1).counter.calls.is_empty());
}

/// 仕様 6.6：切手はほかの商品と一緒でも現金で払う
#[test]
fn stamps_are_paid_in_cash_even_with_other_items() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BREAD);
    scan(&mut s, 1, "floor", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    let v = reg(&s, 1);
    assert!(v.counter.calls.iter().any(|c| c.label == "現金"));
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"floor","method":"emoney"}),
    );
    assert!(run(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"})
    )
    .is_err());
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":500})).is_err());
    login(&mut s, 1);
    assert!(reg(&s, 1).counter.calls.is_empty());

    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    for m in ["emoney", "qr", "credit", "points"] {
        assert!(
            run(
                &mut s,
                json!({"type":"select_payment","register":1,"side":"floor","method":m})
            )
            .is_err(),
            "{m}"
        );
    }
    assert_eq!(
        reg(&s, 1).counter.taxes.iter().map(|t| t.tax).sum::<u32>(),
        0,
        "非課税"
    );

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "floor", BREAD);
    scan(&mut s, 1, "floor", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"emoney"}),
    );
    assert_eq!(reg(&s, 1).cashless_due, 160);
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    let v = reg(&s, 1);
    assert_eq!(emoney_balance(&s), 840);
    assert_eq!(v.floor.due, 110);
    ok(&mut s, json!({"type":"insert","register":1,"amount":500}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.outlet, 390);
    assert_eq!(v.last_receipt.unwrap().method, "電子マネー・現金");

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "floor", BREAD);
    scan(&mut s, 1, "floor", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"emoney"}),
    );
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    assert_eq!(emoney_balance(&s), 1_000);
}

/// 仕様 5.1：買い物点数は 100 点まで
#[test]
fn shopping_is_limited_to_100_items() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", SNACK);
    let id = reg(&s, 1).floor.lines[0].id;
    ok(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"floor","line_id":id,"qty":99}),
    );
    login(&mut s, 1);
    advance(&mut s, 1_000);
    let ff = |qty: u32| json!({"type":"register_ff","register":1,"side":"counter","code":"FF01","qty":qty});
    let e = run(&mut s, ff(2)).unwrap_err();
    assert!(e.contains("100点"), "{e}");
    ok(&mut s, ff(1));
    let v = reg(&s, 1);
    assert_eq!(v.counter.lines.iter().map(|l| l.line.qty).sum::<u32>(), 100);
    assert!(!allowed(&v, Side::Counter, Action::RegisterFf));
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"floor","code":SNACK}),
    )
    .unwrap_err();
    assert!(e.contains("100点"), "{e}");
    let ffid = reg(&s, 1)
        .counter
        .lines
        .iter()
        .map(|l| l.line.id)
        .max()
        .unwrap();
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"counter","line_id":ffid}),
    );
    scan(&mut s, 1, "floor", SNACK);
}

/// 仕様 13：取引中は店舗メニューを開けない
#[test]
fn store_menu_cannot_open_during_a_transaction() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "floor", BREAD);
    let top = json!({"type":"open_menu","register":1,"screen":"top"});
    assert!(run(&mut s, top.clone()).is_err(), "取引中は開けない");
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert!(run(&mut s, top.clone()).is_err(), "会計中も開けない");
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    assert!(reg(&s, 1).menu.is_none());
    login(&mut s, 1);
    ok(&mut s, top);
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SelfOperated);
}

/// 仕様 9：生年月日を入力し、販売できない商品は回収する
#[test]
fn staff_enters_the_birth_date_and_unsellable_items_are_confiscated() {
    let mut s = Store::new(1, 2, 1);
    ok(
        &mut s,
        json!({"type":"set_product_age","owner_id":"000","password":"9999","code":LIGHTER,"min_age":18}),
    );
    login(&mut s, 1);
    for code in [BREAD, BEER, BEER, LIGHTER] {
        scan(&mut s, 1, "counter", code);
    }
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    advance(&mut s, 1_000);
    let v = reg(&s, 1);
    assert!(v.counter.age_choice_pending);
    let birth = |b: &str| json!({"type":"check_id","register":1,"birth":b,"today":"2026-10-19"});
    assert!(run(&mut s, birth("20070501")).is_err());
    assert!(run(
        &mut s,
        json!({"type":"choose_age_check","register":1,"side":"floor","id_check":true})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"choose_age_check","register":1,"id_check":true}),
    );
    let v = reg(&s, 1);
    assert!(v.counter.id_pending && !v.counter.age_choice_pending);
    let cash = json!({"type":"insert_cash","register":1,"denomination":1000,"count":1});
    assert!(run(&mut s, cash.clone()).is_err());
    assert!(run(&mut s, birth("20270101")).is_err());
    let msg = run(&mut s, birth("20070501")).unwrap().unwrap();
    assert!(msg.contains("19歳") && msg.contains("アルコール"), "{msg}");
    let v = reg(&s, 1);
    let codes: Vec<&str> = v.counter.lines.iter().map(|l| l.line.code).collect();
    assert!(!codes.contains(&BEER) && codes.contains(&LIGHTER) && codes.contains(&BREAD));
    assert_eq!(v.counter.confiscated.len(), 1);
    assert_eq!(v.counter.confiscated[0].qty, 2);
    assert!(!v.counter.id_pending && v.counter.phase == Phase::Paying);
    ok(&mut s, json!({"type":"dismiss_confiscated","register":1}));
    assert!(reg(&s, 1).counter.confiscated.is_empty());
    advance(&mut s, 1_000);
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"cash"}),
    );
    ok(&mut s, cash);
}

/// 仕様 9：年齢は誕生日の当日に加算する
#[test]
fn age_counts_the_birthday() {
    use janus_pos_core::store::age_on;
    assert_eq!(age_on("20061019", "2026-10-19"), Ok(20));
    assert_eq!(age_on("20061020", "2026-10-19"), Ok(19));
    assert_eq!(age_on("19821002", "2026-10-19"), Ok(44));
    for bad in ["2006/10/19", "20070231", "570102", "S57.10.2", "2006101"] {
        assert!(age_on(bad, "2026-10-19").is_err(), "{bad}");
    }
}

/// 仕様 5.4：クーポンは対象商品がいつ登録されても効く
#[test]
fn coupons_apply_whenever_the_target_is_registered() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", "CP0001");
    let v = reg(&s, 1);
    assert_eq!(v.floor.coupons.len(), 1);
    assert_eq!(v.floor.coupons[0].discount, None);
    assert_eq!(v.floor.total, 0);
    scan(&mut s, 1, "floor", TEA);
    let v = reg(&s, 1);
    assert_eq!(v.floor.coupons[0].discount, Some(150));
    assert_eq!(v.floor.total, 0);
    scan(&mut s, 1, "floor", "CP0001");
    assert_eq!(reg(&s, 1).floor.coupons[1].discount, None);
    scan(&mut s, 1, "floor", TEA);
    let v = reg(&s, 1);
    assert_eq!(v.floor.coupons[1].discount, Some(150));
    assert_eq!(v.floor.total, 0);
    scan(&mut s, 1, "floor", SANDWICH);
    scan(&mut s, 1, "floor", TEA);
    scan(&mut s, 1, "floor", "CP0002");
    let v = reg(&s, 1);
    assert_eq!(v.floor.total, 150 + 320 - 50);
    assert_eq!(v.counter.lines.iter().map(|l| l.line.qty).sum::<u32>(), 4);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"floor","method":"emoney"}),
    );
    ok(&mut s, json!({"type":"tap_emoney","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.unwrap().total, 420);
}

/// 仕様 5.4：クーポンは適用先の明細に付き、間に別の商品が挟まっても対象商品の明細を指す
#[test]
fn a_coupon_points_at_the_line_it_applies_to() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", SANDWICH);
    scan(&mut s, 1, "floor", TEA);
    scan(&mut s, 1, "floor", "CP0002");
    let v = reg(&s, 1);
    let sandwich = v
        .counter
        .lines
        .iter()
        .find(|l| l.line.code == SANDWICH)
        .unwrap()
        .line
        .id;
    assert_eq!(v.counter.coupons[0].line_id, Some(sandwich));
    assert_eq!(v.floor.coupons[0].line_id, Some(sandwich));
    assert_eq!(v.floor.coupons[0].discount, Some(50));
    scan(&mut s, 1, "floor", "CP0002");
    scan(&mut s, 1, "floor", SANDWICH);
    let v = reg(&s, 1);
    let second = v
        .counter
        .lines
        .iter()
        .filter(|l| l.line.code == SANDWICH)
        .map(|l| l.line.id)
        .max()
        .unwrap();
    assert_ne!(second, sandwich);
    assert_eq!(v.counter.coupons[1].line_id, Some(second));
    ok(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"floor","line_id":sandwich}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.counter.coupons[0].line_id, Some(second));
    assert_eq!(v.counter.coupons[1].line_id, None);
}

/// 仕様 5.4：クーポンは一番近くの対象商品に付く（前に登録した一番新しい明細、なければ後の一番古い明細）
#[test]
fn a_coupon_goes_to_the_nearest_target() {
    const PAN: &str = "4900000000011";
    let mut s = Store::new(1, 2, 1);
    for code in [TEA, PAN, TEA, PAN, TEA] {
        scan(&mut s, 1, "floor", code);
    }
    scan(&mut s, 1, "floor", "CP0001");
    let v = reg(&s, 1);
    let teas: Vec<u32> = v
        .counter
        .lines
        .iter()
        .filter(|l| l.line.code == TEA)
        .map(|l| l.line.id)
        .collect();
    let last = *teas.iter().max().unwrap();
    assert_eq!(v.counter.coupons[0].line_id, Some(last));
    // 先に読んだクーポンは後から登録した一番古い対象商品に付き、さらに後の商品に奪われない
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", "CP0001");
    scan(&mut s, 1, "floor", PAN);
    scan(&mut s, 1, "floor", TEA);
    scan(&mut s, 1, "floor", PAN);
    scan(&mut s, 1, "floor", TEA);
    let v = reg(&s, 1);
    let first = v
        .counter
        .lines
        .iter()
        .filter(|l| l.line.code == TEA)
        .map(|l| l.line.id)
        .min()
        .unwrap();
    assert_eq!(v.counter.coupons[0].line_id, Some(first));
}

/// 仕様 5.4：適用先のないクーポンは返却まで決済を止める
#[test]
fn a_coupon_without_its_target_blocks_payment_until_it_is_returned() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", "CP0001");
    scan(&mut s, 1, "floor", "4900000000028");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    confirm_point(&mut s, 1);
    let v = reg(&s, 1);
    assert!(v.counter.coupon_unmatched && v.floor.coupon_unmatched);
    assert!(v
        .floor
        .payment_blocker
        .unwrap()
        .contains("適用先のないクーポン"));
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"floor","method":"emoney"}),
    );
    assert!(run(&mut s, json!({"type":"tap_emoney","register":1})).is_err());
    let id = reg(&s, 1).floor.coupons[0].id;
    ok(
        &mut s,
        json!({"type":"remove_coupon","register":1,"side":"floor","coupon_id":id}),
    );
    let v = reg(&s, 1);
    assert!(v.floor.coupons.is_empty() && !v.floor.coupon_unmatched);
    assert_eq!(v.counter.returned.len(), 1);
    assert!(v.floor.returned[0].unmatched);
    ok(&mut s, json!({"type":"dismiss_returned","register":1}));
    assert!(reg(&s, 1).counter.returned.is_empty());
    ok(&mut s, json!({"type":"tap_emoney","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.unwrap().total, 320);
}

/// 仕様 5.1：同じ商品は続けて読んだときだけまとめる
#[test]
fn same_item_merges_only_when_scanned_in_a_row() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", SNACK);
    scan(&mut s, 1, "floor", SNACK);
    scan(&mut s, 1, "floor", TEA);
    scan(&mut s, 1, "floor", SNACK);
    scan(&mut s, 1, "floor", SNACK);
    scan(&mut s, 1, "floor", SNACK);
    let v = reg(&s, 1);
    let mut lines: Vec<_> = v.floor.lines.iter().collect();
    lines.sort_by_key(|l| std::cmp::Reverse(l.touched));
    let rows: Vec<_> = lines.iter().map(|l| (l.name, l.qty)).collect();
    assert_eq!(rows, [("袋菓子", 3), ("ソフトドリンク", 1), ("袋菓子", 2)]);
}

/// 仕様 5.1：スタッフもお客様も明細の数量を変えられる
#[test]
fn staff_or_customer_changes_a_line_quantity() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", SNACK);
    scan(&mut s, 1, "floor", SANDWICH);
    let id = reg(&s, 1).floor.lines[0].id;
    let set =
        |qty: u32| json!({"type":"set_qty","register":1,"side":"floor","line_id":id,"qty":qty});
    ok(&mut s, set(3));
    let v = reg(&s, 1);
    let top = v.floor.lines.iter().max_by_key(|l| l.touched).unwrap();
    assert_eq!(top.name, "サンドイッチ", "the changed line keeps its place");
    let bread = v.floor.lines.iter().find(|l| l.id == id).unwrap();
    assert_eq!(bread.qty, 3);
    assert_eq!(v.floor.total, 150 * 3 + 320);
    ok(&mut s, set(99));
    assert!(run(&mut s, set(100)).is_err(), "over the 100-item limit");
    assert!(run(&mut s, set(0)).is_err(), "cancel is a separate button");
    ok(&mut s, set(1));
    assert_eq!(reg(&s, 1).floor.total, 150 + 320);
}

/// 仕様 5.1：クーポンと明細は同じ通し番号を使う
#[test]
fn coupon_and_line_ids_never_collide() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BREAD);
    scan(&mut s, 1, "floor", "4900000000028");
    scan(&mut s, 1, "floor", "CP0002");
    scan(&mut s, 1, "floor", "4900000000011");
    let v = reg(&s, 1);
    let mut ids: Vec<u32> = v.floor.lines.iter().map(|l| l.id).collect();
    ids.extend(v.floor.coupons.iter().map(|c| c.id));
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n);
}

/// 仕様 6.4：スタッフがポイント利用を選べばポイントで払える
#[test]
fn points_can_pay_once_staff_chooses_them() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    scan(&mut s, 1, "counter", "PC12345678");
    assert!(run(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"points"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"points"}),
    );
    use_points(&mut s, 1, "all", None);
    assert!(reg(&s, 1).counter.lines.is_empty());
}

/// 仕様 6.4：カードなしでポイント利用を選ぶとポイントカードの確認に戻る
#[test]
fn points_payment_without_a_card_goes_back_to_the_point_card_check() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    let points = json!({"type":"select_payment","register":1,"side":"counter","method":"points"});
    ok(&mut s, points.clone());
    let v = reg(&s, 1);
    assert!(v.counter.point_pending && v.floor.point_pending);
    assert_eq!(v.counter.method, Some(PaymentMethod::Points));
    scan(&mut s, 1, "counter", "PC12345678");
    let v = reg(&s, 1);
    assert_eq!(
        v.counter.phase,
        Phase::Paying,
        "reading the card keeps the payment screen"
    );
    assert_eq!(v.counter.method, Some(PaymentMethod::Points));
    use_points(&mut s, 1, "all", None);
    let v = reg(&s, 1);
    assert!(v.counter.lines.is_empty());
    assert_eq!(v.last_receipt.map(|r| r.method), Some("ポイント利用"));

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    ok(&mut s, points);
    confirm_point(&mut s, 1);
    let v = reg(&s, 1);
    assert!(!v.counter.point_pending);
    assert_eq!(v.counter.method, None);
}

/// 仕様 11.3：可搬性端末とストアコンピューターの呼び鈴は切れる
#[test]
fn handheld_and_store_computer_bells_can_be_turned_off() {
    let mut s = Store::new(1, 2, 3);
    let ids: Vec<String> = s.snapshot().bells.iter().map(|m| m.id.clone()).collect();
    assert_eq!(ids, ["据え付け端末", "可搬性端末", "ストアコンピューター"]);
    assert!(run(&mut s, json!({"type":"set_bell_on","bell":0,"on":false})).is_err());
    ok(&mut s, json!({"type":"set_bell_on","bell":1,"on":false}));
    ok(&mut s, json!({"type":"set_bell_on","bell":2,"on":false}));
    ok(&mut s, json!({"type":"call_staff","register":1}));
    advance(&mut s, 1_000);
    let snap = s.snapshot();
    assert!(snap.bells[0].buzzing);
    assert!(!snap.bells[1].buzzing && !snap.bells[2].buzzing);
    assert!(!snap.bells[1].bell_on);
    advance(&mut s, 8_000);
    assert!(reg(&s, 1).counter.bell_down.is_empty());
    ok(&mut s, json!({"type":"set_bell_on","bell":2,"on":true}));
    advance(&mut s, 1_000);
    assert!(s.snapshot().bells[2].buzzing);
}

/// 仕様 4.1：たばこの呼び出しはたばこを登録するまで印を残す
#[test]
fn tobacco_call_marks_the_register_until_tobacco_is_scanned() {
    let mut s = Store::new(1, 2, 1);
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"tobacco"}),
    );
    assert!(!reg(&s, 1).tobacco_asked, "self-operated: not yet");
    login(&mut s, 1);
    assert!(reg(&s, 1).tobacco_asked);
    scan(&mut s, 1, "counter", BREAD);
    assert!(reg(&s, 1).tobacco_asked);
    scan(&mut s, 1, "counter", "4900000000127");
    assert!(!reg(&s, 1).tobacco_asked);
    login(&mut s, 2);
    assert!(!reg(&s, 2).tobacco_asked);
}

/// 仕様 6.9：釣銭忘れは案内してからスタッフを呼ぶ
#[test]
fn change_left_in_the_outlet_reminds_then_calls_staff() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"cash"}),
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert!(reg(&s, 1).outlet > 0);
    advance(&mut s, 10_000);
    let frozen = reg(&s, 1).idle_revert_in.unwrap();
    assert!(!reg(&s, 1).change_left);
    advance(&mut s, 21_000);
    let v = reg(&s, 1);
    assert!(
        v.change_left,
        "staff_operated, 30 seconds: remind the customer"
    );
    assert!(v.counter.calls.is_empty());
    assert_eq!(v.idle_revert_in, Some(frozen), "countdown is paused");
    advance(&mut s, 30_000);
    let v = reg(&s, 1);
    assert_eq!(
        v.counter.calls.len(),
        1,
        "staff_operated, 1 minute: call staff"
    );
    assert_eq!(v.counter.calls[0].label, "釣銭忘れ");
    assert!(
        reg(&s, 1).last_receipt.is_some(),
        "stays on the thank-you screen while change is left"
    );
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    let v = reg(&s, 1);
    assert!(!v.change_left && v.counter.calls.is_empty());
    advance(&mut s, 100);
    assert!(
        reg(&s, 1).last_receipt.is_some(),
        "thanks for 3 s after the change is taken"
    );
    advance(&mut s, 3_000);
    assert!(
        reg(&s, 1).last_receipt.is_none(),
        "back to the initial screen"
    );
    let v = reg(&s, 1);
    assert_eq!(
        v.mode,
        Mode::SelfOperated,
        "取引の後始末が済むと担当はログアウトする"
    );
    assert_eq!(v.counter.state, StaffState::Absent);
}

/// 仕様 14.2：オーナーが 1 回のお会計の販売個数の上限を決める
#[test]
fn owner_limits_how_many_of_an_item_one_purchase_can_have() {
    let mut s = Store::new(1, 2, 1);
    let limit = |pw: &str, n: Option<u32>| json!({"type":"set_sale_limit","owner_id":"000","password":pw,"code":TEA,"limit":n});
    assert!(run(&mut s, limit("0000", Some(2))).is_err());
    ok(&mut s, limit("9999", Some(2)));
    let item = s
        .snapshot()
        .catalog
        .into_iter()
        .find(|p| p.code == TEA)
        .unwrap();
    assert_eq!(item.limit, Some(2));
    scan(&mut s, 1, "floor", TEA);
    scan(&mut s, 1, "floor", TEA);
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"floor","code":TEA}),
    )
    .unwrap_err();
    assert!(e.contains("2点まで"), "{e}");
    scan(&mut s, 1, "floor", BREAD);
    let id = reg(&s, 1)
        .floor
        .lines
        .iter()
        .find(|l| l.name == item.name)
        .unwrap()
        .id;
    let set =
        |qty: u32| json!({"type":"set_qty","register":1,"side":"floor","line_id":id,"qty":qty});
    assert!(run(&mut s, set(3)).is_err());
    ok(&mut s, set(1));
    ok(&mut s, set(2));
    ok(&mut s, limit("9999", None));
    scan(&mut s, 1, "floor", TEA);
    assert!(s
        .snapshot()
        .age_rules
        .history
        .iter()
        .any(|h| h.change.contains("2点まで → 制限なし")));
}

/// 仕様 14.3：税率はデータで、変えられるのはオーナーだけ
#[test]
fn tax_rates_are_data_and_owner_only() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BEER);
    let t = reg(&s, 1).counter.taxes;
    assert_eq!(
        t.iter()
            .map(|x| (x.rate.as_str(), x.base, x.tax))
            .collect::<Vec<_>>(),
        vec![("8%", 0, 0), ("10%", 248, 22)]
    );
    assert!(run(
        &mut s,
        json!({"type":"set_tax_rate","owner_id":"001","password":"1111","class_id":"standard","rate_bp":1200})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"set_tax_rate","owner_id":"000","password":"9999","class_id":"standard","rate_bp":1200}),
    );
    scan(&mut s, 1, "counter", TEA);
    scan(&mut s, 1, "counter", BEER);
    let t = reg(&s, 1).counter.taxes;
    let rates: Vec<&str> = t.iter().map(|x| x.rate.as_str()).collect();
    assert_eq!(rates, vec!["8%", "10%", "12%"]);
    assert_eq!(t[2].tax, 248 * 1200 / 11200);
    assert!(run(
        &mut s,
        json!({"type":"set_tax_rate","owner_id":"000","password":"9999","class_id":"zero","rate_bp":500})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"set_invoice","owner_id":"000","password":"9999","invoice_no":"T1234567890123","rounding":"half_up"}),
    );
    assert_eq!(reg(&s, 1).counter.taxes[1].tax, 23);
    assert_eq!(s.snapshot().tax.invoice_no, "T1234567890123");
    assert_eq!(s.snapshot().age_rules.history.len(), 2);
}

/// 仕様 14.3：商品の税区分を変えられる
#[test]
fn product_tax_class_can_be_changed() {
    let mut s = Store::new(1, 2, 1);
    let class = |s: &Store, code: &str| {
        s.snapshot()
            .catalog
            .into_iter()
            .find(|p| p.code == code)
            .unwrap()
            .tax_class
    };
    assert_eq!(class(&s, BILL), "exempt");
    assert_eq!(class(&s, BREAD), "reduced");
    ok(
        &mut s,
        json!({"type":"add_tax_class","owner_id":"000","password":"9999","name":"新税率","kind":"reduced","rate_bp":500}),
    );
    let id = s.snapshot().tax.classes.last().unwrap().id.clone();
    ok(
        &mut s,
        json!({"type":"set_product_tax","owner_id":"000","password":"9999","code":BREAD,"class_id":id}),
    );
    assert_eq!(class(&s, BREAD), id);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    let t = reg(&s, 1).counter.taxes;
    assert!(t
        .iter()
        .any(|x| x.rate == "5%" && x.base == 160 && x.tax == 7));
}

fn emoney_balance(s: &Store) -> u32 {
    s.snapshot().emoney_balance
}

/// 仕様 7：セルフ操作モードのお買い物中の電子マネーはスタッフを呼ぶ
#[test]
fn self_operated_emoney_fan_calls_staff_while_shopping() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BREAD);
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"emoney"}),
    );
    let kinds =
        |s: &Store| -> Vec<CallKind> { reg(s, 1).counter.calls.iter().map(|c| c.kind).collect() };
    assert!(kinds(&s).contains(&CallKind::Emoney));
    login(&mut s, 1);
    assert!(
        !kinds(&s).contains(&CallKind::Emoney),
        "スタッフが来たら片付く"
    );
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"counter","kind":"menu"}),
    );
}

/// 仕様 7：残高確認で残高を出す
#[test]
fn emoney_inquiry_shows_the_balance() {
    let mut s = Store::new(1, 2, 1);
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"floor","kind":"menu"}),
    );
    assert!(
        run(
            &mut s,
            json!({"type":"tap_emoney","register":1,"method":"emoney"})
        )
        .is_err(),
        "先に選ぶ"
    );
    ok(
        &mut s,
        json!({"type":"close_emoney","register":1,"side":"floor"}),
    );
    scan(&mut s, 1, "floor", BREAD);
    for kind in ["menu", "charge", "inquiry"] {
        assert!(
            run(
                &mut s,
                json!({"type":"start_emoney","register":1,"side":"floor","kind":kind})
            )
            .is_err(),
            "お買い物の途中はチャージ・残高確認できない"
        );
    }

    let mut s = Store::new(1, 2, 1);
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"floor","kind":"inquiry"}),
    );
    assert_eq!(reg(&s, 1).emoney.unwrap().balance, None);
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    assert_eq!(reg(&s, 1).emoney.unwrap().balance, Some(1_000));
    ok(
        &mut s,
        json!({"type":"close_emoney","register":1,"side":"floor"}),
    );
    assert!(reg(&s, 1).emoney.is_none());
}

/// 仕様 7：チャージは紙幣で上限まで
#[test]
fn emoney_charge_takes_bills_up_to_the_limit() {
    let mut s = Store::new(1, 2, 1);
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"floor","kind":"inquiry"}),
    );
    assert!(reg(&s, 1).counter.calls.is_empty(), "残高照会は呼ばない");
    ok(
        &mut s,
        json!({"type":"close_emoney","register":1,"side":"floor"}),
    );
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"floor","kind":"charge"}),
    );
    let v = reg(&s, 1);
    assert!(v.emoney.is_none());
    assert!(v.counter.calls.iter().any(|c| c.label == "電子マネー"));
    login(&mut s, 1);
    assert!(reg(&s, 1).counter.calls.is_empty());
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"counter","kind":"charge"}),
    );
    assert!(
        run(&mut s, json!({"type":"insert","register":1,"amount":500})).is_err(),
        "硬貨は使えない"
    );
    assert!(
        run(
            &mut s,
            json!({"type":"tap_emoney","register":1,"method":"emoney"})
        )
        .is_err(),
        "まだ入れていない"
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":10000}));
    ok(&mut s, json!({"type":"insert","register":1,"amount":5000}));
    assert!(
        run(&mut s, json!({"type":"insert","register":1,"amount":5000})).is_err(),
        "上限を超える"
    );
    assert_eq!(reg(&s, 1).emoney.unwrap().inserted, 15_000);
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    assert_eq!(emoney_balance(&s), 16_000);
    assert!(reg(&s, 1).emoney.is_none());
    assert_eq!(
        reg(&s, 1).mode,
        Mode::SelfOperated,
        "チャージを終えると担当はログアウトする"
    );

    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"counter","kind":"charge"}),
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":2000}));
    ok(
        &mut s,
        json!({"type":"close_emoney","register":1,"side":"counter"}),
    );
    assert_eq!(reg(&s, 1).outlet, 2000);
    assert_eq!(emoney_balance(&s), 16_000);
}

fn short_checkout(s: &mut Store) {
    short_checkout_on(s, "floor");
}

fn short_checkout_on(s: &mut Store, side: &str) {
    for _ in 0..7 {
        scan(s, 1, side, BREAD);
    }
    ok(s, json!({"type":"checkout","register":1,"side":side}));
    confirm_point(s, 1);
    ok(
        s,
        json!({"type":"select_payment","register":1,"side":side,"method":"emoney"}),
    );
    ok(
        s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    let v = reg(s, 1);
    assert_eq!(v.emoney_short, Some(1_000));
    assert!(v.last_receipt.is_none());
    assert!(
        !allowed(&v, Side::Floor, Action::InsertCash),
        "選ぶまで払えない"
    );
}

/// 仕様 6.7：電子マネーの残高不足は三つの選択を出す
#[test]
fn emoney_short_offers_three_choices() {
    let mut s = Store::new(1, 2, 1);
    short_checkout(&mut s);
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"floor","choice":"cancel"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.emoney_short, None);
    assert_eq!(v.floor.method, None);
    assert_eq!(emoney_balance(&s), 1_000);

    let mut s = Store::new(1, 2, 1);
    short_checkout(&mut s);
    assert!(run(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"floor","choice":"cash"})
    )
    .is_err());

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    short_checkout_on(&mut s, "counter");
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"counter","choice":"cash"}),
    );
    assert_eq!(reg(&s, 1).emoney_partial, Some(1_000));
    assert!(
        run(
            &mut s,
            json!({"type":"tap_emoney","register":1,"method":"emoney"})
        )
        .is_err(),
        "案内を確認するまでは引き落とさない（かざしたままでも）"
    );
    assert_eq!(emoney_balance(&s), 1_000);
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"counter","choice":"cash_confirmed"}),
    );
    assert!(
        run(&mut s, json!({"type":"insert","register":1,"amount":1000})).is_err(),
        "先に電子マネーをかざす"
    );
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.emoney_partial, None);
    assert_eq!(v.floor.due, 120);
    assert_eq!(v.prepaid, 1_000);
    assert_eq!(emoney_balance(&s), 0);
    assert!(
        run(
            &mut s,
            json!({"type":"back_to_registering","register":1,"side":"counter"})
        )
        .is_err(),
        "電子マネーで払った後は登録に戻れない"
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.outlet, 880);
    assert_eq!(v.last_receipt.unwrap().method, "電子マネー・現金");

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    short_checkout_on(&mut s, "counter");
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"counter","choice":"cash"}),
    );
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"counter","choice":"cancel"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.emoney_partial, None);
    assert_eq!(v.floor.method, None);
    assert_eq!(emoney_balance(&s), 1_000);

    let mut s = Store::new(1, 2, 1);
    short_checkout(&mut s);
    assert!(
        run(
            &mut s,
            json!({"type":"resolve_emoney_short","register":1,"side":"floor","choice":"charge"})
        )
        .is_err(),
        "セミセルフはお買い物の途中にチャージしない"
    );
    assert_eq!(reg(&s, 1).emoney_short, Some(1_000));

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    short_checkout_on(&mut s, "counter");
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"counter","choice":"charge"}),
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    assert_eq!(emoney_balance(&s), 2_000);
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    assert_eq!(reg(&s, 1).last_receipt.unwrap().total, 1_120);
    assert_eq!(emoney_balance(&s), 880);
}

/// 仕様 12.1：お客様が取引中止するとお金が戻る
#[test]
fn customer_voids_and_money_comes_back() {
    let mut s = Store::new(1, 2, 1);
    assert!(
        run(&mut s, json!({"type":"void_transaction","register":1})).is_err(),
        "取引がない"
    );
    scan(&mut s, 1, "floor", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    confirm_point(&mut s, 1);
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":100})).is_err());
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    let v = reg(&s, 1);
    assert!(v.floor.lines.is_empty());
    assert_eq!(v.outlet, 0);
}

/// 仕様 12.3：店舗メニューからログアウト・休止する
#[test]
fn logout_and_pause_from_either_store_menu() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    open_staff_menu(&mut s, 1);
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SelfOperated);
    assert!(v.menu.is_none());
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    assert!(run(&mut s, json!({"type":"pause","register":1})).is_err());
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    login(&mut s, 1);
    open_staff_menu(&mut s, 1);
    ok(&mut s, json!({"type":"pause","register":1}));
    let v = reg(&s, 1);
    assert!(v.paused);
    assert_eq!(v.mode, Mode::SelfOperated);
    assert!(v.menu.is_none());
    assert!(run(
        &mut s,
        json!({"type":"scan","register":1,"side":"floor","code":BREAD})
    )
    .is_err());
    assert!(run(&mut s, json!({"type":"call_staff","register":1})).is_err());
    scan(&mut s, 1, "counter", "ST001");
    let v = reg(&s, 1);
    assert!(!v.paused);
    assert_eq!(v.mode, Mode::StaffOperated);
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":"top"}),
    );
    ok(&mut s, json!({"type":"pause","register":1}));
    assert!(reg(&s, 1).paused);
}

/// 仕様 8：お渡し済みの FF を増やすと照合に戻り、減らすと回収を求める
#[test]
fn changing_handed_ff_rewinds_or_asks_to_collect() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"counter","code":"FF01","qty":2}),
    );
    let id = reg(&s, 1).counter.lines[0].line.id;
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"counter","line_id":id,"handed":2}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"counter","line_id":id,"qty":3}),
    );
    let v = reg(&s, 1);
    assert!(matches!(
        v.counter.lines[0].line.kind,
        LineKind::Ff {
            state: FfState::Waiting
        }
    ));
    assert!(!allowed(&v, Side::Counter, Action::Checkout));
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"counter","line_id":id,"handed":3}),
    );
    assert!(matches!(
        reg(&s, 1).counter.lines[0].line.kind,
        LineKind::Ff {
            state: FfState::Waiting
        }
    ));
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"counter","line_id":id,"handed":1}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"counter","line_id":id,"qty":2}),
    );
    let v = reg(&s, 1);
    assert!(matches!(
        v.counter.lines[0].line.kind,
        LineKind::Ff {
            state: FfState::Received
        }
    ));
    assert_eq!(v.counter.ff_collect.len(), 1);
    assert_eq!(v.counter.ff_collect[0].qty, 1);
    ok(&mut s, json!({"type":"dismiss_ff_collect","register":1}));
    assert!(reg(&s, 1).counter.ff_collect.is_empty());
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"counter","line_id":id}),
    );
    assert_eq!(reg(&s, 1).counter.ff_collect[0].qty, 2);
}

/// 仕様 5.5：たばこ・切手・公共料金・荷物にはポイントを付けない
#[test]
fn points_are_not_earned_on_tobacco_stamps_bills_or_parcels() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", "PC12345678");
    scan(&mut s, 1, "counter", TOBACCO);
    scan(&mut s, 1, "counter", STAMP);
    scan(&mut s, 1, "counter", BREAD);
    let bread = reg(&s, 1)
        .counter
        .lines
        .iter()
        .find(|l| l.line.code == BREAD)
        .map(|l| l.amount)
        .unwrap();
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    no_id_check(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"cash"}),
    );
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":10000,"count":1}),
    );
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.map(|r| r.points), Some(bread / 200));

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert!(!reg(&s, 1).counter.point_pending);
}

/// 仕様 8：FF のお渡し照合はスタッフだけが行う
#[test]
fn staff_alone_is_responsible_for_ff_handover() {
    let mut s = Store::new(1, 1, 1);
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"counter","code":"FF01","qty":2}),
    );
    let id = reg(&s, 1).counter.lines[0].line.id;
    advance(&mut s, 1_000);
    assert!(!allowed(&reg(&s, 1), Side::Counter, Action::Checkout));
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"counter","line_id":id,"handed":2}),
    );
    advance(&mut s, 1_000);
    assert!(allowed(&reg(&s, 1), Side::Counter, Action::Checkout));
    ok(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"counter","line_id":id,"qty":3}),
    );
    advance(&mut s, 1_000);
    assert!(!allowed(&reg(&s, 1), Side::Counter, Action::Checkout));
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"counter","line_id":id,"handed":1}),
    );
    advance(&mut s, 1_000);
    assert!(allowed(&reg(&s, 1), Side::Counter, Action::Checkout));
}

/// 仕様 6.5：足りた状態で 30 秒入れなければ投入おわり
#[test]
fn thirty_seconds_without_insert_means_done_inserting() {
    let mut s = Store::new(1, 2, 1);
    s.tick(1000);
    login(&mut s, 1);
    scan(&mut s, 1, "floor", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    assert_eq!(reg(&s, 1).counter.method, None, "method not chosen yet");
    ok(&mut s, json!({"type":"insert","register":1,"amount":100}));
    assert_eq!(
        reg(&s, 1).counter.method,
        Some(PaymentMethod::Cash),
        "inserting skips the method choice"
    );
    s.tick(1000 + 31_000);
    assert!(reg(&s, 1).last_receipt.is_none());
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    s.tick(1000 + 31_000 + 29_999);
    assert!(reg(&s, 1).last_receipt.is_none(), "still within 30 s");
    s.tick(1000 + 31_000 + 30_000);
    let v = reg(&s, 1);
    assert!(v.last_receipt.is_some(), "settled automatically");
    assert_eq!(v.outlet, 1100 - 160);
}

/// 仕様 8：ログアウト後は FF のお渡しを照合できない
#[test]
fn ff_handover_cannot_be_confirmed_after_staff_logs_out() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"counter","code":"FF01","qty":1}),
    );
    let id = reg(&s, 1).counter.lines[0].line.id;
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    advance(&mut s, 2_000);
    for side in ["floor", "counter"] {
        assert!(
            run(
                &mut s,
                json!({"type":"check_ff","register":1,"side":side,"line_id":id,"handed":1})
            )
            .is_err(),
            "{side}"
        );
    }
    assert!(matches!(
        reg(&s, 1).counter.lines[0].line.kind,
        LineKind::Ff {
            state: FfState::Waiting
        }
    ));
}

fn same_after_reload(s: &Store) -> Store {
    let saved = s.save_json();
    let back = Store::load_json(&saved).unwrap_or_else(|e| panic!("load failed: {e}"));
    assert_eq!(
        serde_json::to_value(s.snapshot()).unwrap(),
        serde_json::to_value(back.snapshot()).unwrap()
    );
    assert_eq!(saved, back.save_json());
    back
}

/// 仕様 3.4：保存した状態からすべてのレジを戻す
#[test]
fn saved_state_restores_every_register() {
    let mut s = Store::new(7, 2, 3);
    let set = json!({"type":"set_product_age","owner_id":"000","password":"9999","code":SANDWICH,"min_age":20});
    ok(&mut s, set);
    scan(&mut s, 2, "floor", BEER);
    scan(&mut s, 2, "floor", TEA);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"floor"}),
    );
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    scan(&mut s, 1, "counter", SANDWICH);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    advance(&mut s, 1500);
    let mut back = same_after_reload(&s);

    for st in [&mut s, &mut back] {
        no_id_check(st, 1);
        confirm_point(st, 1);
        ok(st, json!({"type":"insert","register":1,"amount":1000}));
        ok(st, json!({"type":"finish_insert","register":1}));
        advance(st, 3000);
    }
    assert_eq!(
        serde_json::to_value(s.snapshot()).unwrap(),
        serde_json::to_value(back.snapshot()).unwrap(),
        "restored store keeps running the same way"
    );
    same_after_reload(&back);
}

/// 仕様 3.4：壊れた・古い保存状態は読み込まない
#[test]
fn broken_or_old_saved_state_is_refused() {
    assert!(Store::load_json("{").is_err());
    let s = Store::new(1, 2, 1);
    let mut v: Value = serde_json::from_str(&s.save_json()).unwrap();
    v["schema"] = json!(0);
    assert!(Store::load_json(&v.to_string()).is_err());
}

/// 仕様 8：FF の在庫は四段階で見せ、欠品は予約注文できる
#[test]
fn ff_stock_is_shown_as_four_levels_and_out_of_stock_can_be_reserved() {
    use janus_pos_core::store::StockLevel;
    let mut s = Store::new(1, 1, 1);
    let level = |s: &Store, code: &str| {
        s.snapshot()
            .ff_stock
            .iter()
            .find(|f| f.code == code)
            .unwrap()
            .level
    };
    assert_eq!(level(&s, "FF31"), StockLevel::MadeToOrder);
    assert_eq!(level(&s, "FF51"), StockLevel::MadeToOrder);
    assert_eq!(level(&s, "FF03"), StockLevel::Plenty);
    assert_eq!(level(&s, "FF62"), StockLevel::Low);
    assert_eq!(level(&s, "FF23"), StockLevel::Out);
    assert_eq!(level(&s, "FF24"), StockLevel::Out);
    login_as(&mut s, 1, "ST001");
    let e = run(
        &mut s,
        json!({"type":"register_ff","register":1,"code":"FF63","qty":1}),
    )
    .unwrap_err();
    assert!(e.contains("欠品") && !e.contains("残り"), "{e}");
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"code":"FF63","qty":1,"force":true}),
    );
    assert_eq!(level(&s, "FF63"), StockLevel::Out);
    assert_eq!(reg(&s, 1).counter.lines.len(), 1);
    for _ in 0..3 {
        ok(
            &mut s,
            json!({"type":"register_ff","register":1,"code":"FF62","qty":1}),
        );
    }
    assert_eq!(level(&s, "FF62"), StockLevel::Out);
    // 豊富（二桁）から一つ減ると希薄（一桁）になる
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"code":"FF61","qty":1}),
    );
    assert_eq!(level(&s, "FF61"), StockLevel::Low);
    // 作り置きの無い商品は何杯でも登録でき、表示は変わらない
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"code":"FF31","qty":5}),
    );
    assert_eq!(level(&s, "FF31"), StockLevel::MadeToOrder);
}

/// 仕様 6.5：現金投入後のバックで現金を返し、支払方法の選択に戻る
#[test]
fn back_after_inserting_cash_returns_it_and_reopens_methods() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BREAD);
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"counter"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.outlet, 1000, "unsettled cash goes to the outlet");
    assert_eq!(v.counter.inserted, 0);
    assert_eq!(v.counter.method, None, "back to choosing a method");
    assert_eq!(
        v.counter.phase,
        Phase::Paying,
        "stays on the payment screen"
    );
    assert!(v.last_receipt.is_none());
    advance(&mut s, 40_000);
    assert!(reg(&s, 1).last_receipt.is_none(), "nothing settles itself");
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"emoney"}),
    );
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"counter"}),
    );
    assert_eq!(reg(&s, 1).counter.phase, Phase::Registering);
}

/// 仕様 6.4：選んだ支払方法をもう一度押すと選択を外す
#[test]
fn pressing_the_chosen_payment_method_again_clears_it() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    confirm_point(&mut s, 1);
    let pick = json!({"type":"select_payment","register":1,"side":"floor","method":"emoney"});
    ok(&mut s, pick.clone());
    assert_eq!(reg(&s, 1).counter.method, Some(PaymentMethod::Emoney));
    ok(&mut s, pick);
    assert_eq!(reg(&s, 1).counter.method, None);
    let e = run(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    )
    .unwrap_err();
    assert!(!e.is_empty());
}

/// 仕様 5.5：ポイントカードの確認は一度きりで、決済まではいつでも読める
#[test]
fn point_card_is_asked_once_but_can_be_read_until_paid() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert!(reg(&s, 1).counter.point_pending);
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"qr"}),
    );
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"counter"}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    let v = reg(&s, 1);
    assert!(!v.counter.point_pending, "declined once: never asked again");
    assert!(v.counter.point_card.is_none());
    scan(&mut s, 1, "floor", "PC12345678");
    let v = reg(&s, 1);
    assert_eq!(
        v.counter.phase,
        Phase::Paying,
        "reading the card keeps the payment screen"
    );
    assert!(v.counter.point_card.is_some());
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":500,"count":1}),
    );
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.map(|r| r.points), Some(160 / 200));
}

/// 仕様 9：登録中に年齢確認でき、新しい年齢の商品はやり直す
#[test]
fn staff_can_check_age_early_and_new_age_goods_need_it_again() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BEER);
    scan(&mut s, 1, "counter", BREAD);
    assert!(allowed(&reg(&s, 1), Side::Counter, Action::EarlyAgeCheck));
    assert!(run(
        &mut s,
        json!({"type":"early_age_check","register":1,"side":"floor","start":true})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"early_age_check","register":1,"side":"counter","start":true}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.counter.phase, Phase::Registering);
    assert!(v.counter.age_choice_pending);
    assert_eq!(v.counter.required_age, Some(20));
    ok(
        &mut s,
        json!({"type":"choose_age_check","register":1,"id_check":true}),
    );
    ok(
        &mut s,
        json!({"type":"check_id","register":1,"birth":"20100101","today":"2026-11-09"}),
    );
    let v = reg(&s, 1);
    assert!(!v.counter.age_early);
    assert_eq!(v.counter.lines.len(), 1, "beer is removed early");
    assert!(!allowed(&v, Side::Counter, Action::EarlyAgeCheck));

    scan(&mut s, 1, "counter", LIGHTER);
    let v = reg(&s, 1);
    assert!(
        !v.counter
            .lines
            .iter()
            .find(|l| l.age.is_some())
            .unwrap()
            .age_checked
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert!(
        reg(&s, 1).counter.age_choice_pending,
        "a lighter added after the check needs the check again"
    );
}

/// 仕様 9：早めの年齢確認はやめられ、済めば会計で尋ねない
#[test]
fn early_age_check_can_be_cancelled_and_covers_checkout() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BEER);
    ok(
        &mut s,
        json!({"type":"early_age_check","register":1,"side":"counter","start":true}),
    );
    ok(
        &mut s,
        json!({"type":"early_age_check","register":1,"side":"counter","start":false}),
    );
    assert!(!reg(&s, 1).counter.age_choice_pending);
    ok(
        &mut s,
        json!({"type":"early_age_check","register":1,"side":"counter","start":true}),
    );
    no_id_check(&mut s, 1);
    let v = reg(&s, 1);
    assert!(v.counter.lines[0].age_checked);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert!(
        !reg(&s, 1).counter.age_choice_pending,
        "checked early, not asked again"
    );
}

fn ring(s: &mut Store, r: u8) {
    ok(s, json!({"type":"call_staff","register":r}));
    advance(s, 200);
    ok(s, json!({"type":"release_call","register":r}));
    advance(s, 200);
}

/// 呼び鈴を 3 分以内に 30 回鳴らしたところ（非常事態モードに入る）
fn enter_emergency(s: &mut Store, r: u8) {
    for _ in 0..EMERGENCY_PRESSES {
        ring(s, r);
    }
    assert!(reg(s, r).emergency);
}

/// 仕様 11.4：3 分以内に 30 回鳴らすと入る。29 回では入らない
#[test]
fn emergency_after_thirty_rings_within_three_minutes() {
    let mut s = Store::new(1, 1, 1);
    advance(&mut s, 2_000);
    for _ in 0..EMERGENCY_PRESSES - 1 {
        ring(&mut s, 1);
    }
    assert!(!reg(&s, 1).emergency, "29 rings");
    ring(&mut s, 1);
    assert!(reg(&s, 1).emergency, "30 rings");
}

/// 仕様 11.4：30 回でも 3 分を超えて間をあけて鳴らしたものは数えない
#[test]
fn rings_spread_over_more_than_three_minutes_do_not_count() {
    let mut s = Store::new(1, 1, 1);
    let gap = EMERGENCY_WINDOW_MS / (EMERGENCY_PRESSES as u64 - 1) + 1_000;
    for _ in 0..EMERGENCY_PRESSES {
        ring(&mut s, 1);
        advance(&mut s, gap);
    }
    assert!(!reg(&s, 1).emergency);
}

/// 仕様 11.4：3 分間鳴らしっぱなし（押しっ放し）にすると入る。手前で離せば入らない
#[test]
fn emergency_when_the_bell_is_held_for_three_minutes() {
    let mut s = Store::new(1, 1, 1);
    ok(&mut s, json!({"type":"call_staff","register":1}));
    advance(&mut s, EMERGENCY_WINDOW_MS - 1_000);
    assert!(!reg(&s, 1).emergency, "2:59");
    advance(&mut s, 1_000);
    assert!(reg(&s, 1).emergency, "3:00");
    ok(&mut s, json!({"type":"release_call","register":1}));
    assert!(reg(&s, 1).emergency, "lasts after release");
}

/// 仕様 11.4：押しっ放しを手前で離すと数え直す
#[test]
fn releasing_before_three_minutes_resets_the_hold() {
    let mut s = Store::new(1, 1, 1);
    ok(&mut s, json!({"type":"call_staff","register":1}));
    advance(&mut s, EMERGENCY_WINDOW_MS - 1_000);
    ok(&mut s, json!({"type":"release_call","register":1}));
    ok(&mut s, json!({"type":"call_staff","register":1}));
    advance(&mut s, 2_000);
    assert!(!reg(&s, 1).emergency);
}

/// 仕様 11.4：呼び出しに応答がないまま時間がたっても入らない（お客様が自己解決しただけで誤って発動しないように）
#[test]
fn an_unanswered_call_alone_is_not_an_emergency() {
    let mut s = Store::new(1, 1, 1);
    ring(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"tobacco"}),
    );
    ok(&mut s, json!({"type":"release_call","register":1}));
    advance(&mut s, 30 * 60 * 1000);
    let v = reg(&s, 1);
    assert!(!v.waiting_calls.is_empty());
    assert!(!v.emergency);
}

/// 仕様 11.4：ログインで鳴らした回数は数え直す
#[test]
fn login_clears_the_ring_count() {
    let mut s = Store::new(1, 1, 1);
    for _ in 0..EMERGENCY_PRESSES - 1 {
        ring(&mut s, 1);
    }
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"end_staff_mode","register":1,"side":"counter"}),
    );
    ring(&mut s, 1);
    assert!(!reg(&s, 1).emergency);
}

/// 仕様 11.4：呼び鈴が不通でも入る（スタッフが応答しない状況には違いない）
#[test]
fn emergency_also_while_the_bell_cannot_reach_anyone() {
    let mut s = Store::new(1, 1, 1);
    ok(
        &mut s,
        json!({"type":"set_bell_offline","bell":0,"offline":true}),
    );
    advance(&mut s, 20_000);
    assert!(reg(&s, 1).bell_dead);
    enter_emergency(&mut s, 1);
}

/// 仕様 11.4：スタッフ操作モードでは入らない
#[test]
fn no_emergency_in_staff_operated_mode() {
    let mut s = Store::new(1, 1, 1);
    login(&mut s, 1);
    for _ in 0..10 {
        ok(
            &mut s,
            json!({"type":"call_staff","register":1,"topic":"sos"}),
        );
        advance(&mut s, 200);
        ok(&mut s, json!({"type":"release_call","register":1}));
    }
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"sos"}),
    );
    advance(&mut s, EMERGENCY_WINDOW_MS);
    assert!(!reg(&s, 1).emergency);
}

/// 仕様 11.4：解除はログインなしで誰でもでき、呼び鈴の呼び出しも片付いて通常運転に戻る
#[test]
fn anyone_can_end_emergency_without_logging_in() {
    let mut s = Store::new(1, 1, 1);
    enter_emergency(&mut s, 1);
    ok(&mut s, json!({"type":"end_emergency","register":1}));
    let v = reg(&s, 1);
    assert!(!v.emergency);
    assert!(v.emergency_log.is_some());
    assert_eq!(v.mode, Mode::SelfOperated);
    assert!(!v.lamp);
    assert!(v.waiting_calls.is_empty());
}

/// 仕様 11.4：解除すると鳴らした回数を数え直す（解除の直後に一回押しても入り直さない）
#[test]
fn ending_emergency_clears_the_ring_count() {
    let mut s = Store::new(1, 1, 1);
    enter_emergency(&mut s, 1);
    ok(&mut s, json!({"type":"end_emergency","register":1}));
    ring(&mut s, 1);
    assert!(!reg(&s, 1).emergency);
}

/// 仕様 11.4：ログインだけでは続き、解除で発動・解除時刻のレシートを出して通常運転に戻る
#[test]
fn emergency_lasts_until_ended() {
    let mut s = Store::new(1, 1, 1);
    enter_emergency(&mut s, 1);
    advance(&mut s, 300_000);
    assert!(reg(&s, 1).emergency);
    let since = reg(&s, 1).emergency_since.unwrap();
    login(&mut s, 1);
    assert!(reg(&s, 1).emergency);
    advance(&mut s, 5_000);
    ok(&mut s, json!({"type":"end_emergency","register":1}));
    let v = reg(&s, 1);
    assert!(!v.emergency);
    let log = v.emergency_log.unwrap();
    assert_eq!(log.started_at, since);
    assert!(log.ended_at > since);
    advance(&mut s, EMERGENCY_LOG_MS);
    assert!(reg(&s, 1).emergency_log.is_none());
    assert_eq!(reg(&s, 1).mode, Mode::StaffOperated);
    advance(&mut s, janus_pos_core::store::IDLE_REVERT_MS);
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SelfOperated);
    assert!(!v.emergency);
}

/// 仕様 6.5：二千円札は出金ボックスに返す
#[test]
fn two_thousand_yen_bill_is_returned_to_the_outlet() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    // 二千円札そのものは受け付けず、出金ボックスへ
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":2000,"count":1}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.counter.inserted, 0);
    assert_eq!(v.outlet, 2000);
    assert_eq!(v.outlet_detail, vec![(2000, 1)]);
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    // テンキーで 2000 と打つのは千円札 2 枚とみなす
    ok(&mut s, json!({"type":"insert","register":1,"amount":2000}));
    let v = reg(&s, 1);
    assert_eq!(v.counter.inserted, 2000);
    assert_eq!(v.outlet, 0);
}

const TICKET: &str = "2100000000074";

/// 仕様 10.5：チケットは支払い後に発券し、枚数が一致したらお渡しを終える
#[test]
fn tickets_are_issued_after_payment_and_handed_over_when_the_customer_count_matches() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", TICKET);
    for code in [TICKET, BREAD] {
        let e = run(
            &mut s,
            json!({"type":"scan","register":1,"side":"counter","code":code}),
        )
        .unwrap_err();
        assert!(e.contains("一件ずつ"), "{code}: {e}");
    }
    let v = reg(&s, 1);
    assert_eq!(v.counter.lines.len(), 1, "チケットは一件ずつ");
    assert_eq!(v.counter.total, 15_600);
    assert!(v.ticket_handover.is_empty(), "支払い前は発券しない");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert!(
        !reg(&s, 1).counter.point_pending,
        "ポイント対象外なのでカードの確認はない"
    );
    for m in ["credit", "qr"] {
        assert!(
            run(
                &mut s,
                json!({"type":"select_payment","register":1,"side":"counter","method":m})
            )
            .is_err(),
            "チケットは現金のみ: {m}"
        );
    }
    ok(&mut s, json!({"type":"insert","register":1,"amount":16000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.last_receipt.as_ref().unwrap().points, 0);
    let rows: Vec<_> = v
        .ticket_handover
        .iter()
        .map(|t| (t.name, t.issued))
        .collect();
    assert_eq!(rows, [("チケット", 2)]);
    assert_eq!(v.outlet, 0, "お渡しの確認が済むまでお釣りは出さない");
    advance(&mut s, 10_000);
    assert!(
        reg(&s, 1).last_receipt.is_some(),
        "お渡しが済むまでお礼の画面に戻らない"
    );
    assert!(
        receive_tickets(&mut s, 1, "counter", 2).is_err(),
        "受け取り枚数は売り場側で入力する"
    );
    let m = receive_tickets(&mut s, 1, "floor", 3).unwrap().unwrap();
    assert!(m.contains("一致しません"), "{m}");
    let v = reg(&s, 1);
    assert_eq!(v.tickets_received, Some(3));
    assert_eq!(v.ticket_handover.len(), 1, "枚数が合うまで終わらない");
    assert!(
        hand_tickets(&mut s, 1, "floor", 2).is_err(),
        "お渡し枚数はレジカウンター側で入力する"
    );
    let m = hand_tickets(&mut s, 1, "counter", 1).unwrap().unwrap();
    assert!(m.contains("発券枚数とお渡し枚数が一致しません"), "{m}");
    let m = hand_tickets(&mut s, 1, "counter", 2).unwrap().unwrap();
    assert!(
        !m.contains("一致しません"),
        "スタッフの入力は発券枚数と比べる: {m}"
    );
    assert_eq!(reg(&s, 1).tickets_handed, Some(2));
    assert_eq!(
        reg(&s, 1).ticket_handover.len(),
        1,
        "お客様の入力が発券枚数と一致するまで終わらない"
    );
    let m = receive_tickets(&mut s, 1, "floor", 2).unwrap().unwrap();
    assert!(m.contains("チケット 2枚をお渡ししました"), "{m}");
    let v = reg(&s, 1);
    assert!(v.ticket_handover.is_empty());
    assert_eq!(v.outlet, 16_000 - 15_600, "お渡しが済んでからお釣りを出す");
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    advance(&mut s, 10_000);
    assert!(reg(&s, 1).last_receipt.is_none());
}

/// 仕様 10.5：チケットはほかの商品と同じ取引にできない
#[test]
fn tickets_cannot_share_a_transaction_with_other_goods() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"counter","code":TICKET}),
    )
    .unwrap_err();
    assert!(e.contains("チケット"), "{e}");
}

/// 仕様 10.5：セルフ操作モードのチケットは現金でスタッフを呼ぶ
#[test]
fn self_operated_ticket_checkout_calls_staff_for_cash() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", TICKET);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    let calls = s.snapshot().calls;
    assert!(
        calls
            .iter()
            .any(|c| c.register == 1 && c.kind == CallKind::CashOnly && c.detail == "チケット"),
        "{calls:?}"
    );
}

fn receive_tickets(
    s: &mut Store,
    r: u8,
    side: &str,
    received: u32,
) -> Result<Option<String>, String> {
    run(
        s,
        json!({"type":"receive_tickets","register":r,"side":side,"received":received}),
    )
}

fn hand_tickets(s: &mut Store, r: u8, side: &str, handed: u32) -> Result<Option<String>, String> {
    run(
        s,
        json!({"type":"hand_tickets","register":r,"side":side,"handed":handed}),
    )
}

fn pay_tickets(s: &mut Store, r: u8) {
    scan(s, r, "counter", TICKET);
    ok(s, json!({"type":"checkout","register":r,"side":"counter"}));
    ok(s, json!({"type":"insert","register":r,"amount":16000}));
    ok(s, json!({"type":"finish_insert","register":r}));
}

/// 仕様 10.5：チケットは店全体で先入れ先出しに発券する
#[test]
fn tickets_are_issued_first_in_first_out_across_registers() {
    let mut s = Store::new(3, 3, 1);
    login_as(&mut s, 1, "ST001");
    login_as(&mut s, 2, "ST002");
    login_as(&mut s, 3, "ST000");
    pay_tickets(&mut s, 2);
    pay_tickets(&mut s, 1);
    let (r1, r2) = (reg(&s, 1), reg(&s, 2));
    assert!(
        r2.tickets_issued && r2.tickets_ahead == 0,
        "先に支払ったレジ2から発券"
    );
    assert!(
        !r1.tickets_issued && r1.tickets_ahead == 1,
        "レジ1は発券待ち"
    );
    assert!(
        receive_tickets(&mut s, 1, "floor", 2).is_err(),
        "発券前は受け取り枚数を入力できない"
    );
    // 発券を待っているあいだも、レジ1はお渡しが済むまで次の取引を始められない
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"counter","code":BREAD}),
    )
    .unwrap_err();
    assert!(e.contains("チケットのお渡し"), "{e}");
    pay_tickets(&mut s, 3);
    assert_eq!(reg(&s, 3).tickets_ahead, 2, "後から支払ったレジ3は列の最後");
    receive_tickets(&mut s, 2, "floor", 2).unwrap();
    assert!(
        !reg(&s, 2).ticket_handover.is_empty(),
        "スタッフの入力がまだ"
    );
    hand_tickets(&mut s, 2, "counter", 2).unwrap();
    let (r1, r2) = (reg(&s, 1), reg(&s, 2));
    assert!(r2.ticket_handover.is_empty());
    assert!(
        r1.tickets_issued && r1.tickets_ahead == 0,
        "先のお渡しが済んだので次を発券"
    );
    hand_tickets(&mut s, 1, "counter", 2).unwrap();
    receive_tickets(&mut s, 1, "floor", 2).unwrap();
    assert!(reg(&s, 1).ticket_handover.is_empty());
    scan(&mut s, 1, "counter", BREAD);
}

/// 仕様 3.4：発券の列は保存と読み込みで残る
#[test]
fn ticket_queue_survives_save_and_load() {
    let mut s = Store::new(2, 2, 1);
    login_as(&mut s, 1, "ST001");
    login_as(&mut s, 2, "ST002");
    pay_tickets(&mut s, 1);
    pay_tickets(&mut s, 2);
    let s = Store::load_json(&s.save_json()).unwrap();
    assert!(reg(&s, 1).tickets_issued);
    assert!(!reg(&s, 2).tickets_issued && reg(&s, 2).tickets_ahead == 1);
}

/// 仕様 5.1：日配食品は数量を増やせない
#[test]
fn fresh_food_quantity_cannot_be_raised_on_screen() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    let id = reg(&s, 1).counter.lines[0].line.id;
    let set =
        |qty: u32| json!({"type":"set_qty","register":1,"side":"counter","line_id":id,"qty":qty});
    let e = run(&mut s, set(2)).unwrap_err();
    assert!(e.contains("日配食品"), "{e}");
    assert!(reg(&s, 1).counter.lines[0].fresh);
    // 続けて読んでも数量にまとめず、一点ずつ別の行になる
    scan(&mut s, 1, "counter", BREAD);
    let v = reg(&s, 1);
    assert_eq!(v.counter.lines.len(), 2);
    assert!(v.counter.lines.iter().all(|l| l.line.qty == 1));
    // 加工食品は数量を変えられる
    scan(&mut s, 1, "counter", SNACK);
    let snack = reg(&s, 1)
        .counter
        .lines
        .iter()
        .find(|l| l.line.code == SNACK)
        .unwrap()
        .line
        .id;
    ok(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"counter","line_id":snack,"qty":3}),
    );
}

/// 仕様 6.3：販売期限切れの品は取り除くまで会計できない
#[test]
fn expired_onigiri_blocks_checkout_until_removed() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    let m = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"counter","code":ONIGIRI}),
    )
    .unwrap()
    .unwrap();
    assert!(m.contains("販売期限が切れています"), "{m}");
    let v = reg(&s, 1);
    let l = v
        .counter
        .lines
        .iter()
        .find(|l| l.line.code == ONIGIRI)
        .unwrap();
    assert!(l.expired && l.fresh);
    assert!(v.floor.lines.iter().any(|l| l.expired));
    assert!(!allowed(&v, Side::Counter, Action::Checkout));
    let id = l.line.id;
    ok(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"counter","line_id":id}),
    );
    let v = reg(&s, 1);
    assert!(allowed(&v, Side::Counter, Action::Checkout));
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
}

/// 仕様 11.1：セルフ操作モードで期限切れを読むとスタッフを呼ぶ
#[test]
fn self_operated_expired_scan_calls_staff() {
    let mut s = Store::new(1, 2, 1);
    let m = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"floor","code":ONIGIRI}),
    )
    .unwrap()
    .unwrap();
    assert!(m.contains("スタッフをお呼びしました"), "{m}");
    assert!(s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.kind == CallKind::Expired));
    let id = reg(&s, 1).floor.lines[0].id;
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    )
    .is_err());
    // セルフの売り場側には、期限切れの明細を消す手段がない（B-3）
    assert!(run(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"floor","line_id":id}),
    )
    .is_err());
    assert!(run(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"floor","line_id":id,"qty":0}),
    )
    .is_err());
    assert_eq!(reg(&s, 1).floor.lines.len(), 1);
    // 消せるのはレジカウンター側の「回収しました」だけ
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"counter","line_id":id}),
    );
    assert!(reg(&s, 1).counter.lines.is_empty());
}

fn open_calls(s: &Store, r: u8) -> Vec<CallKind> {
    reg(s, r).waiting_calls
}

/// 仕様 4.1：販売期限切れを読み、ログインして回収せずにログアウトすると「販売期限切れ」の呼び出しが立ち直る（B-3）
#[test]
fn expired_call_comes_back_after_logout_without_collecting() {
    let mut s = Store::new(1, 1, 1);
    scan(&mut s, 1, "floor", ONIGIRI);
    assert_eq!(open_calls(&s, 1), vec![CallKind::Expired]);
    login(&mut s, 1);
    assert!(open_calls(&s, 1).is_empty());
    ok(
        &mut s,
        json!({"type":"end_staff_mode","register":1,"side":"counter"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SelfOperated);
    assert_eq!(v.waiting_calls, vec![CallKind::Expired]);
    assert!(v.lamp, "the customer side shows the staff call");
}

/// 仕様 4.1：会計後の年齢確認も、ログインして確認せずにログアウトすると「年確」の呼び出しが立ち直る（B-3）
#[test]
fn age_check_call_comes_back_after_logout_without_checking() {
    let mut s = Store::new(1, 1, 1);
    scan(&mut s, 1, "floor", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    assert!(open_calls(&s, 1).contains(&CallKind::AgeCheck));
    login(&mut s, 1);
    assert!(open_calls(&s, 1).is_empty());
    ok(
        &mut s,
        json!({"type":"end_staff_mode","register":1,"side":"counter"}),
    );
    assert!(open_calls(&s, 1).contains(&CallKind::AgeCheck));
}

/// 仕様 4.1：公共料金・受付待ちのサービス商品も、取引に残っていればログアウトで立ち直る（B-3）
#[test]
fn bill_and_service_calls_come_back_after_logout() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", WATER_BILL);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    assert!(open_calls(&s, 1).contains(&CallKind::Bill));
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"end_staff_mode","register":1,"side":"counter"}),
    );
    assert!(open_calls(&s, 1).contains(&CallKind::Bill));

    scan(&mut s, 2, "floor", PARCEL);
    assert!(open_calls(&s, 2).contains(&CallKind::Service));
    login(&mut s, 2);
    ok(
        &mut s,
        json!({"type":"end_staff_mode","register":2,"side":"counter"}),
    );
    assert!(open_calls(&s, 2).contains(&CallKind::Service));
}

const CARTON: &str = "4900000000158";
const FIREWORKS: &str = "4910000000076";

/// 仕様 9：確認済みの年齢の商品では年齢確認をやり直さない
#[test]
fn goods_needing_an_age_already_checked_do_not_redo_the_check() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", LIGHTER);
    ok(
        &mut s,
        json!({"type":"early_age_check","register":1,"side":"counter","start":true}),
    );
    no_id_check(&mut s, 1);
    scan(&mut s, 1, "counter", CARTON);
    let v = reg(&s, 1);
    assert!(
        v.counter.lines.iter().all(|l| l.age_checked),
        "20 is already checked"
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert!(
        !reg(&s, 1).counter.age_choice_pending,
        "a carton after a lighter (both 20) does not redo the check"
    );
}

/// 仕様 9：20 歳の確認の後の 18 歳の商品はやり直す
#[test]
fn an_18_item_after_a_20_check_redoes_the_check_for_18() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    no_id_check(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"counter"}),
    );
    scan(&mut s, 1, "counter", FIREWORKS);
    let v = reg(&s, 1);
    let fw = v.counter.lines.iter().find(|l| l.age == Some(18)).unwrap();
    assert!(!fw.age_checked);
    assert!(
        v.counter
            .lines
            .iter()
            .find(|l| l.age == Some(20))
            .unwrap()
            .age_checked
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    let v = reg(&s, 1);
    assert!(v.counter.age_choice_pending);
    assert_eq!(v.counter.required_age, Some(18));
    no_id_check(&mut s, 1);
    assert!(reg(&s, 1).counter.lines.iter().all(|l| l.age_checked));
}

/// 仕様 6.7：セルフ操作モードの残高不足は頼まれたときだけスタッフを呼ぶ
#[test]
fn self_operated_emoney_short_calls_staff_only_when_asked() {
    let mut s = Store::new(1, 2, 1);
    short_checkout(&mut s);
    assert_eq!(reg(&s, 1).emoney_short, Some(1_000));
    assert!(
        reg(&s, 1).counter.calls.is_empty(),
        "残高不足の時点では呼ばない"
    );
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"floor","choice":"staff"}),
    );
    assert_eq!(reg(&s, 1).emoney_short, Some(1_000));
    assert!(reg(&s, 1)
        .counter
        .calls
        .iter()
        .any(|c| c.label == "電子マネー"));
    assert!(
        run(
            &mut s,
            json!({"type":"resolve_emoney_short","register":1,"side":"floor","choice":"cancel"})
        )
        .is_err(),
        "呼んだあとは他の決済に切り替えない（スタッフの無駄足になる）"
    );
    assert!(reg(&s, 1)
        .counter
        .calls
        .iter()
        .any(|c| c.label == "電子マネー"));

    // スタッフがログインすると、スタッフ操作モードの残高不足の画面につながり、不足額を現金で払える
    let mut s = Store::new(1, 2, 1);
    short_checkout(&mut s);
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"floor","choice":"staff"}),
    );
    login(&mut s, 1);
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::StaffOperated);
    assert_eq!(v.emoney_short, Some(1_000));
    assert!(v.counter.calls.is_empty());
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"counter","choice":"cash"}),
    );
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"counter","choice":"cash_confirmed"}),
    );
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.unwrap().method, "電子マネー・現金");
}

fn pay_cash(s: &mut Store, r: u8) {
    ok(
        s,
        json!({"type":"select_payment","register":r,"side":"counter","method":"cash"}),
    );
    ok(
        s,
        json!({"type":"insert_cash","register":r,"denomination":10000,"count":1}),
    );
    ok(s, json!({"type":"finish_insert","register":r}));
}

/// 仕様 5.5：付与対象 200 円につき 1P を付け、カードに残す
#[test]
fn points_are_earned_per_200_yen_of_eligible_goods_and_kept_on_the_card() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", "PC12345678");
    scan(&mut s, 1, "counter", SANDWICH);
    scan(&mut s, 1, "counter", SANDWICH);
    scan(&mut s, 1, "counter", TOBACCO);
    let v = reg(&s, 1);
    assert!(v.points.eligible);
    assert_eq!(v.points.balance, Some(1_250));
    assert_eq!(v.points.earn, 640 / 200, "tobacco earns nothing");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    no_id_check(&mut s, 1);
    pay_cash(&mut s, 1);
    let r = reg(&s, 1).last_receipt.expect("決済済み");
    assert_eq!(
        (r.points, r.points_used, r.point_balance),
        (3, 0, Some(1_253))
    );

    scan(&mut s, 1, "counter", BREAD);
    scan(&mut s, 1, "counter", "PC12345678");
    assert_eq!(reg(&s, 1).points.balance, Some(1_253));
}

/// 仕様 6.4：ポイント残高が足りなければ一部を払い、残りは別の支払方法
#[test]
fn a_short_point_balance_pays_part_and_the_rest_goes_to_another_method() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", "PC87654321");
    scan(&mut s, 1, "counter", SANDWICH);
    scan(&mut s, 1, "counter", TOBACCO);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    no_id_check(&mut s, 1);
    assert_eq!(reg(&s, 1).points.usable, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"points"}),
    );
    use_points(&mut s, 1, "all", None);
    let v = reg(&s, 1);
    assert_eq!((v.prepaid, v.counter.due), (1, 919));
    assert_eq!(v.counter.method, None);
    assert_eq!(
        v.points.earn,
        (320 - 1) / 200,
        "no points on the part paid by points"
    );
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"credit"}),
    );
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"credit"}),
    );
    let r = reg(&s, 1).last_receipt.expect("決済済み");
    assert_eq!((r.points, r.points_used, r.point_balance), (1, 1, Some(1)));
}

/// 仕様 6.4：取引中止で使ったポイントを戻し、残高 0 では払えない
#[test]
fn voiding_returns_used_points_and_an_empty_card_cannot_pay() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", "PC87654321");
    scan(&mut s, 1, "counter", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"points"}),
    );
    use_points(&mut s, 1, "all", None);
    assert_eq!(reg(&s, 1).points.balance, Some(0));
    ok(
        &mut s,
        json!({"type":"void_transaction","register":1,"side":"counter"}),
    );
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    scan(&mut s, 1, "counter", "PC87654321");
    assert_eq!(reg(&s, 1).points.balance, Some(1));

    // 1P を使い切ると、残高 0 のカードではポイント利用を選べない
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"points"}),
    );
    use_points(&mut s, 1, "all", None);
    pay_cash(&mut s, 1);
    assert_eq!(
        reg(&s, 1).last_receipt.and_then(|r| r.point_balance),
        Some(0)
    );
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    scan(&mut s, 1, "counter", "PC87654321");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert!(reg(&s, 1)
        .unavailable
        .iter()
        .any(|u| u.method == PaymentMethod::Points));
    assert!(run(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"counter","method":"points"})
    )
    .is_err());
}

/// 仕様 5.5：付与対象がない取引だけポイントカードを確認しない
#[test]
fn only_non_point_goods_skip_the_point_card_check() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", TOBACCO);
    assert!(!reg(&s, 1).points.eligible);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    no_id_check(&mut s, 1);
    assert!(!reg(&s, 1).counter.point_pending);
}

fn use_points(s: &mut Store, r: u8, how: &str, points: Option<u32>) {
    ok(
        s,
        json!({"type":"use_points","register":r,"side":"counter","how":how,"points":points}),
    );
}

/// 仕様 6.4：ポイントは全部・端数・指定額で使える
#[test]
fn points_can_pay_all_the_odd_yen_or_a_chosen_amount() {
    let start = |s: &mut Store| {
        login(s, 1);
        scan(s, 1, "counter", "PC12345678");
        scan(s, 1, "counter", SANDWICH);
        scan(s, 1, "counter", BREAD);
        scan(s, 1, "counter", TOBACCO);
        ok(s, json!({"type":"checkout","register":1,"side":"counter"}));
        no_id_check(s, 1);
        ok(
            s,
            json!({"type":"select_payment","register":1,"side":"counter","method":"points"}),
        );
    };
    // 合計 1,080円（うちポイントを使えるのは 480円）
    let mut s = Store::new(1, 2, 1);
    start(&mut s);
    let v = reg(&s, 1);
    assert_eq!(v.prepaid, 0, "choosing points alone uses nothing yet");
    assert_eq!((v.points.usable, v.points.fraction), (480, 80));
    use_points(&mut s, 1, "fraction", None);
    let v = reg(&s, 1);
    assert_eq!((v.prepaid, v.counter.due), (80, 1_000));

    let mut s = Store::new(1, 2, 1);
    start(&mut s);
    assert!(run(
        &mut s,
        json!({"type":"use_points","register":1,"side":"counter","how":"amount","points":481})
    )
    .is_err());
    use_points(&mut s, 1, "amount", Some(300));
    let v = reg(&s, 1);
    assert_eq!((v.prepaid, v.counter.due), (300, 780));
    assert_eq!(v.points.balance, Some(950));

    let mut s = Store::new(1, 2, 1);
    start(&mut s);
    use_points(&mut s, 1, "all", None);
    assert_eq!(reg(&s, 1).prepaid, 480);
}

/// 仕様 6.8：0 円の取引は会計で支払方法を選ばずに終わる
#[test]
fn a_zero_yen_purchase_finishes_on_checkout_without_choosing_a_payment() {
    let mut s = Store::new(1, 2, 1);
    login_as(&mut s, 2, "ST001");
    scan(&mut s, 2, "counter", "4900000000035");
    scan(&mut s, 2, "counter", "CP0001");
    assert_eq!(reg(&s, 2).floor.total, 0);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"counter"}),
    );
    let v = reg(&s, 2);
    assert!(v.counter.lines.is_empty(), "決済手段を選ばずに取引が終わる");
    assert_eq!(v.last_receipt.unwrap().method, "お支払いなし");
}

/// 仕様 10.1：複数口は届け先を共有し、サイズだけ尋ねる
#[test]
fn multi_piece_parcels_share_the_address_and_only_ask_for_the_size() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", PARCEL_MULTI);
    login_as(&mut s, 2, "ST001");
    let id = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id,"postal":"1000001"}),
    );
    let v = reg(&s, 2);
    assert!(v.counter.parcel_ask && v.floor.parcel_ask);
    assert!(!allowed(&v, Side::Counter, Action::Checkout));
    ok(
        &mut s,
        json!({"type":"parcel_next","register":2,"more":true}),
    );
    let id2 = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id2,"sum_cm":58,"weight_g":1500}),
    );
    let pc = |s: &Store, i: usize| reg(s, 2).counter.lines[i].line.parcel.clone().unwrap();
    assert!(pc(&s, 1).multi && !pc(&s, 1).collect);
    assert_eq!(pc(&s, 1).postal, pc(&s, 0).postal);
    assert_ne!(pc(&s, 1).slip, pc(&s, 0).slip);
    assert!(reg(&s, 2).counter.parcel_ask);
    ok(
        &mut s,
        json!({"type":"parcel_next","register":2,"more":false}),
    );
    let v = reg(&s, 2);
    assert!(!v.counter.parcel_ask);
    assert_eq!(v.floor.total, 940 * 2);
    assert!(allowed(&v, Side::Counter, Action::Checkout));
}

/// 仕様 10.1：お客様控えは伝票ごとに一枚、複数口は一枚
#[test]
fn customer_copies_are_one_per_slip_and_a_multi_piece_slip_is_one() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "floor", PARCEL_COLLECT);
    login_as(&mut s, 2, "ST001");
    let id = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id,"postal":"1000001"}),
    );
    scan(&mut s, 2, "counter", PARCEL_MULTI);
    for i in 0..3 {
        let id = pending_parcel(&s, 2);
        ok(
            &mut s,
            json!({"type":"parcel_size","register":2,"line_id":id,"sum_cm":58,"weight_g":1500}),
        );
        if i == 0 {
            ok(
                &mut s,
                json!({"type":"parcel_postal","register":2,"line_id":id,"postal":"5300001"}),
            );
        }
        ok(
            &mut s,
            json!({"type":"parcel_next","register":2,"more":i < 2}),
        );
    }
    assert_eq!(reg(&s, 2).counter.lines.len(), 4);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"counter"}),
    );
    ok(
        &mut s,
        json!({"type":"select_payment","register":2,"side":"counter","method":"cash"}),
    );
    ok(
        &mut s,
        json!({"type":"insert_cash","register":2,"denomination":10000,"count":1}),
    );
    ok(&mut s, json!({"type":"finish_insert","register":2}));
    let v = reg(&s, 2);
    assert_eq!(v.parcel_handover, Some(2), "着払い1枚＋複数口1枚");
    ok(
        &mut s,
        json!({"type":"hand_over_bills","register":2,"side":"counter"}),
    );
    assert_eq!(reg(&s, 2).parcel_handover, None);
}

/// 仕様 5.5：ポイントカードだけでも取引が始まり、取引中止できる
#[test]
fn a_point_card_alone_starts_a_transaction_that_can_be_voided() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "floor", "PC12345678");
    let v = reg(&s, 1);
    assert!(v.floor.point_card);
    assert!(allowed(&v, Side::Floor, Action::VoidTransaction));
    ok(
        &mut s,
        json!({"type":"void_transaction","register":1,"side":"floor"}),
    );
    assert!(!reg(&s, 1).floor.point_card);
}

/// 仕様 14.2：トレカは既定で 1 回のお会計 3 パックまで
#[test]
fn trading_cards_sell_at_most_three_packs_by_default() {
    let mut s = Store::new(1, 2, 1);
    let card = janus_pos_core::catalog::TRADING_CARD;
    // 既定の販売規則：トレカは3点まで、花火は（法令ではなく）店舗の判断で年確18
    let snap = s.snapshot();
    let find = |code: &str| {
        snap.catalog
            .iter()
            .find(|p| p.code == code)
            .unwrap()
            .clone()
    };
    assert_eq!(find(card).limit, Some(3));
    let fireworks = find("4910000000076");
    assert_eq!((fireworks.store_age, fireworks.legal_age), (Some(18), None));
    for _ in 0..3 {
        scan(&mut s, 1, "floor", card);
    }
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"floor","code":card}),
    )
    .unwrap_err();
    assert!(e.contains("3点まで"), "{e}");
    let qty: u32 = reg(&s, 1)
        .floor
        .lines
        .iter()
        .filter(|l| l.name == "トレカ")
        .map(|l| l.qty)
        .sum();
    assert_eq!(qty, 3);
    // ブラウザのアラートではなく、理由（一行目＝上限・二行目＝売れないこと）を返して登録画面の中央に一瞬重ねる
    assert!(e.ends_with("\n3点を超える分はお売りできません"), "{e}");
    scan(&mut s, 1, "floor", BREAD);
    // 上限を超えようとした明細にだけ個数制限のフラグ（お会計は止めない）
    let r = reg(&s, 1);
    assert!(
        r.floor
            .lines
            .iter()
            .find(|l| l.name == "トレカ")
            .unwrap()
            .limit_hit
    );
    assert!(
        !r.floor
            .lines
            .iter()
            .find(|l| l.name != "トレカ")
            .unwrap()
            .limit_hit
    );
}

/// 仕様 5.6：上限を超えて登録しようとした明細にだけ限定n を付ける
#[test]
fn trading_cards_are_flagged_only_once_over_the_limit() {
    let mut s = Store::new(1, 2, 1);
    let card = janus_pos_core::catalog::TRADING_CARD;
    login(&mut s, 1);
    for _ in 0..3 {
        scan(&mut s, 1, "counter", card);
    }
    assert!(!reg(&s, 1).counter.lines[0].limit_hit);
    assert!(run(
        &mut s,
        json!({"type":"scan","register":1,"side":"counter","code":card})
    )
    .is_err());
    assert!(reg(&s, 1).counter.lines[0].limit_hit);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
}

/// 仕様 11.3：応答のない呼び出しは 3 回まで鳴らし直す
#[test]
fn unanswered_call_rings_again_up_to_three_times() {
    use janus_pos_core::store::{BELL_REMINDS, BELL_REMIND_MS};
    let mut s = Store::new(3, 2, 1);
    advance(&mut s, 3_000);
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"tobacco"}),
    );
    advance(&mut s, 50);
    ok(&mut s, json!({"type":"release_call","register":1}));
    advance(&mut s, 2_000);
    let first = s.snapshot().bells[0].rings;
    assert_eq!(first, 1);
    advance(&mut s, BELL_REMIND_MS - 3_000);
    assert_eq!(
        s.snapshot().bells[0].rings,
        first,
        "not before the interval"
    );
    advance(&mut s, 3_000);
    assert_eq!(
        s.snapshot().bells[0].rings,
        first + 1,
        "rings again after the interval"
    );
    advance(&mut s, BELL_REMIND_MS);
    assert_eq!(s.snapshot().bells[0].rings, first + 2);
    advance(&mut s, BELL_REMIND_MS);
    assert_eq!(s.snapshot().bells[0].rings, first + 3);
    advance(&mut s, BELL_REMIND_MS * 4);
    assert_eq!(
        s.snapshot().bells[0].rings,
        first + BELL_REMINDS as u64,
        "rings again at most BELL_REMINDS times"
    );
    // 鳴らし直しは非常事態モードの鳴らした回数に数えない
    assert!(!reg(&s, 1).emergency, "reminders are not customer presses");
    // 新しい呼び出しがあれば、また BELL_REMINDS 回まで鳴らし直す
    ok(&mut s, json!({"type":"ff_button","register":1}));
    ok(&mut s, json!({"type":"release_call","register":1}));
    advance(&mut s, 2_000);
    let again = s.snapshot().bells[0].rings;
    advance(&mut s, BELL_REMIND_MS);
    assert_eq!(s.snapshot().bells[0].rings, again + 1);
    login_as(&mut s, 1, "ST001");
    let r = s.snapshot().bells[0].rings;
    advance(&mut s, BELL_REMIND_MS * 2);
    assert_eq!(s.snapshot().bells[0].rings, r, "stops once staff logs in");
}

/// 仕様 11.1：呼び出しはログインまで出す
#[test]
fn customer_call_shows_until_login() {
    let mut s = Store::new(3, 2, 1);
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"tobacco"}),
    );
    ok(&mut s, json!({"type":"release_call","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.waiting_calls, vec![CallKind::Tobacco]);

    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"tobacco"}),
    );
    ok(&mut s, json!({"type":"release_call","register":1}));
    login_as(&mut s, 1, "ST001");
    let v = reg(&s, 1);
    assert!(v.waiting_calls.is_empty());
}

/// 仕様 11.1：セルフ操作モードのチャージの呼び出しもほかと同じく待つ
#[test]
fn self_emoney_charge_call_waits_like_other_waiting_calls() {
    let mut s = Store::new(3, 2, 1);
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"floor","kind":"charge"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.waiting_calls, vec![CallKind::Emoney]);
    login_as(&mut s, 1, "ST001");
    let v = reg(&s, 1);
    assert!(v.waiting_calls.is_empty());
}

/// 仕様 11.1：セルフ操作モードの呼び出しは最下段で待ち、ログインで解ける
#[test]
fn every_self_call_waits_on_the_bottom_row_until_login() {
    // 年確
    let mut s = Store::new(3, 2, 1);
    ok(
        &mut s,
        json!({"type":"scan","register":1,"side":"floor","code":"4900000000103"}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    assert!(reg(&s, 1).waiting_calls.contains(&CallKind::AgeCheck));
    login_as(&mut s, 1, "ST001");
    let v = reg(&s, 1);
    assert!(v.waiting_calls.is_empty());
    // 宅配便（受付中のサービス商品が呼ぶサービス）。用件は「サービス」ではなく「宅配便」
    let mut s = Store::new(3, 2, 1);
    ok(
        &mut s,
        json!({"type":"scan","register":1,"side":"floor","code":"2100000000014"}),
    );
    assert_eq!(reg(&s, 1).waiting_calls, vec![CallKind::Service]);
    assert_eq!(reg(&s, 1).counter.calls[0].topic, "宅配便");
    login_as(&mut s, 1, "ST001");
    assert!(reg(&s, 1).waiting_calls.is_empty());
}

/// 仕様 11.1：用件は具体的
#[test]
fn call_topic_is_concrete() {
    // 公共料金の払込票は「お支払い」
    let mut s = Store::new(3, 2, 1);
    ok(
        &mut s,
        json!({"type":"scan","register":1,"side":"floor","code":"2100000000043"}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"floor"}),
    );
    assert_eq!(reg(&s, 1).counter.calls[0].topic, "お支払い");
    // 最下段のボタンはボタンの文言そのまま
    for (topic, want) in [
        ("voucher", "金券"),
        ("other", "その他"),
        ("tobacco", "たばこ"),
    ] {
        let mut s = Store::new(3, 2, 1);
        ok(
            &mut s,
            json!({"type":"call_staff","register":1,"topic":topic}),
        );
        ok(&mut s, json!({"type":"release_call","register":1}));
        assert_eq!(reg(&s, 1).counter.calls[0].topic, want);
    }
}

/// 仕様 9.1：年確・FF の照合が済んだ案内はスタッフが次へ進むまで出す
#[test]
fn passing_age_or_ff_check_is_shown_until_staff_moves_on() {
    use janus_pos_core::store::Passed;
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    ok(
        &mut s,
        json!({"type":"choose_age_check","register":1,"side":"counter","id_check":true}),
    );
    assert_eq!(reg(&s, 1).counter.passed, None);
    ok(
        &mut s,
        json!({"type":"check_id","register":1,"birth":"19900101","today":"2026-11-14"}),
    );
    assert_eq!(reg(&s, 1).counter.passed, Some(Passed::Age { age: 20 }));
    ok(&mut s, json!({"type":"dismiss_passed","register":1}));
    assert_eq!(reg(&s, 1).counter.passed, None);

    // 年齢が足りないときは照合OKではなく回収
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BEER);
    scan(&mut s, 1, "counter", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    ok(
        &mut s,
        json!({"type":"choose_age_check","register":1,"side":"counter","id_check":true}),
    );
    ok(
        &mut s,
        json!({"type":"check_id","register":1,"birth":"20100101","today":"2026-11-14"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.counter.passed, None);
    assert_eq!(v.counter.confiscated.len(), 1);

    // FF：最後の一品が一致したときだけ照合OK
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"counter","code":"FF01","qty":2}),
    );
    let ff = reg(&s, 1).counter.lines[0].line.id;
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"counter","line_id":ff,"handed":1}),
    );
    assert_eq!(reg(&s, 1).counter.passed, None);
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"counter","line_id":ff,"handed":2}),
    );
    assert_eq!(reg(&s, 1).counter.passed, Some(Passed::Ff));
}

/// 仕様 9：店舗の年齢制限は顔写真付きの身分証、法令の年齢制限は公的な身分証
#[test]
fn store_set_age_limits_accept_photo_ids_but_legal_ones_need_official_ids() {
    const FIREWORKS: &str = "4910000000076";
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", FIREWORKS);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert!(
        !reg(&s, 1).counter.age_official_id,
        "fireworks is a store rule"
    );
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"counter"}),
    );
    scan(&mut s, 1, "counter", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    assert!(
        reg(&s, 1).counter.age_official_id,
        "alcohol is a legal rule"
    );
}

/// 仕様 15.3：受け付けなかった操作は理由を返し、続く操作は妨げない（ソフトドリンクの後の荷物の伝票）（B-2）
#[test]
fn a_rejected_scan_returns_its_reason_and_the_next_scan_goes_through() {
    let mut s = Store::new(1, 1, 1);
    scan(&mut s, 1, "floor", TEA);
    let err = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"floor","code":PARCEL}),
    )
    .unwrap_err();
    assert!(
        err.contains("お荷物同士なら、何個でも一緒に会計できます"),
        "{err}"
    );
    scan(&mut s, 1, "floor", BREAD);
    assert_eq!(reg(&s, 1).floor.lines.len(), 2);
}

/// 仕様 6.5：現金の投入中、両画面の帯に出す合計（残りのお支払い額）・お預かり・お釣り／不足の元になる値（B-4）
#[test]
fn tendered_cash_and_change_are_shown_on_both_sides() {
    let mut s = Store::new(1, 1, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", SANDWICH);
    scan(&mut s, 1, "counter", SANDWICH);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"counter"}),
    );
    confirm_point(&mut s, 1);
    ok(&mut s, json!({"type":"insert","register":1,"amount":500}));
    let v = reg(&s, 1);
    assert_eq!(
        (v.counter.total - v.prepaid, v.counter.inserted),
        (640, 500)
    );
    assert_eq!(v.counter.due, 140, "不足 140 円");
    ok(&mut s, json!({"type":"insert","register":1,"amount":500}));
    let v = reg(&s, 1);
    assert_eq!(v.counter.inserted, 1000);
    assert_eq!(
        v.counter.inserted - (v.counter.total - v.prepaid),
        360,
        "お釣り 360 円"
    );
    ok(
        &mut s,
        json!({"type":"finish_insert","register":1,"side":"counter"}),
    );
    let v = reg(&s, 1);
    let rc = v.last_receipt.unwrap();
    assert_eq!((rc.paid, rc.change_total), (1000, 360));
}

/// 仕様 12.2：呼び出し中は無操作の残り時間が減らず、片付いたら止まった値から続きを数える（B-5）
#[test]
fn idle_countdown_pauses_while_a_call_is_open() {
    use janus_pos_core::store::IDLE_REVERT_MS;
    let mut s = Store::new(1, 1, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "counter", BREAD);
    advance(&mut s, 60_000);
    let left = reg(&s, 1).idle_revert_in.unwrap();
    assert_eq!(left, IDLE_REVERT_MS - 60_000);
    // ログアウトしてお客様がベルを押す（ログアウトも操作なので、ここから数え直す）
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    ok(&mut s, json!({"type":"call_staff","register":1}));
    ok(&mut s, json!({"type":"release_call","register":1}));
    advance(&mut s, 200);
    let held = reg(&s, 1).idle_revert_in.unwrap();
    advance(&mut s, 120_000);
    assert_eq!(
        reg(&s, 1).idle_revert_in.unwrap(),
        held,
        "frozen while calling"
    );
    // ログインで片付くと、止まった値から続きを数える（ログインも操作なので満タンに戻る）
    login(&mut s, 1);
    let after = reg(&s, 1).idle_revert_in.unwrap();
    advance(&mut s, 10_000);
    assert_eq!(reg(&s, 1).idle_revert_in.unwrap(), after - 10_000);
}

/// 仕様 12.2：セルフ操作モードの呼び出しのあいだも残り時間は変わらない（B-5）
#[test]
fn idle_countdown_is_frozen_for_a_self_operated_call() {
    let mut s = Store::new(1, 1, 1);
    scan(&mut s, 1, "floor", BREAD);
    advance(&mut s, 10_000);
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"tobacco"}),
    );
    ok(&mut s, json!({"type":"release_call","register":1}));
    let held = reg(&s, 1).idle_revert_in.unwrap();
    advance(&mut s, 200_000);
    assert_eq!(reg(&s, 1).idle_revert_in.unwrap(), held);
    assert_eq!(
        reg(&s, 1).floor.lines.len(),
        1,
        "the transaction is not voided"
    );
}
