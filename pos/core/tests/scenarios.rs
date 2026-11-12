use janus_pos_core::authority::{Action, Mode, PaymentMethod, Phase, Side, StaffState};
use janus_pos_core::calls::CallKind;
use janus_pos_core::store::{Command, FfState, LineKind, RegisterView, Store};
use serde_json::{json, Value};

const BREAD: &str = "4900000000011";
const BEER: &str = "4900000000103";
const LIGHTER: &str = "4900000000134";
const SANDWICH: &str = "4900000000028";
const TEA: &str = "4900000000035";
const PARCEL: &str = "2100000000014";
const PARCEL_COLLECT: &str = "2100000000045";
const RECEIVE: &str = "2100000000038";
const BILL: &str = "2100000000021";
const WATER_BILL: &str = "2100000000043";
const GAS_BILL: &str = "2100000000050";
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

fn login(s: &mut Store, r: u8) {
    login_as(s, r, "ST001");
}

fn open_staff_menu(s: &mut Store, r: u8) {
    ok(
        s,
        json!({"type":"open_menu","register":r,"side":"staff","screen":"top"}),
    );
}

fn login_as(s: &mut Store, r: u8, badge: &str) {
    scan(s, r, "staff", badge);
    ok(s, json!({"type":"claim","register":r,"side":"staff"}));
}

fn no_id_check(s: &mut Store, r: u8) {
    ok(
        s,
        json!({"type":"choose_age_check","register":r,"id_check":false}),
    );
}

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

#[test]
fn any_barcode_is_accepted_in_any_order() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    scan(&mut s, 1, "customer", "PC12345678");
    scan(&mut s, 1, "staff", "2000000000015");
    scan(&mut s, 1, "customer", BREAD);
    let v = reg(&s, 1);
    assert_eq!(v.staff.lines.len(), 3);
    assert_eq!(v.staff.total, 160 * 2 + 3);
    assert!(v.customer.point_card);
    assert!(run(
        &mut s,
        json!({"type":"scan","register":1,"side":"staff","code":"0000"})
    )
    .is_err());
}

#[test]
fn scanning_on_payment_screen_returns_to_registering() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":100,"count":1}),
    );
    scan(&mut s, 1, "customer", BREAD);
    let v = reg(&s, 1);
    assert_eq!(v.staff.phase, Phase::Registering);
    assert_eq!(v.staff.inserted, 100);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    assert!(!reg(&s, 1).staff.point_pending);
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":500,"count":1}),
    );
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.last_receipt.as_ref().unwrap().change_total, 600 - 320);
}

#[test]
fn age_check_happens_at_checkout_and_blocks_payment() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BEER);
    assert_eq!(
        reg(&s, 1).staff.required_age,
        None,
        "no age check at scan time"
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    let v = reg(&s, 1);
    assert!(v.staff.age_choice_pending);
    assert!(!v.staff.point_pending);
    assert!(run(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":1000,"count":1})
    )
    .is_err());
    no_id_check(&mut s, 1);
    assert!(!reg(&s, 1).staff.age_choice_pending);
    confirm_point(&mut s, 1);
    scan(&mut s, 1, "staff", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    assert!(
        !reg(&s, 1).staff.point_pending,
        "point card asked once per txn"
    );
    assert!(!reg(&s, 1).staff.age_choice_pending);
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":1000,"count":1}),
    );
}

#[test]
fn self_operated_age_check_calls_staff() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"customer"}),
    );
    assert!(s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.register == 2 && c.kind == CallKind::AgeCheck));
    assert!(!allowed(
        &reg(&s, 2),
        Side::Customer,
        Action::ChooseAgeCheck
    ));
    assert_eq!(reg(&s, 2).customer.age_wait, Some(20));
    scan(&mut s, 2, "staff", "ST002");
    assert_eq!(reg(&s, 2).customer.age_wait, None);
    no_id_check(&mut s, 2);
    assert_eq!(reg(&s, 2).mode, Mode::StaffOperated);
    assert_eq!(reg(&s, 2).staff.state, StaffState::Ready);
    assert!(s.snapshot().calls.is_empty());
}

#[test]
fn ff_button_works_while_age_check_call_is_open() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"customer"}),
    );
    let v = reg(&s, 2);
    assert!(allowed(&v, Side::Customer, Action::FfButton));
    assert!(allowed(&v, Side::Customer, Action::CallStaff));
    ok(&mut s, json!({"type":"ff_button","register":2}));
    let calls = s.snapshot().calls;
    assert!(calls
        .iter()
        .any(|c| c.register == 2 && c.kind == CallKind::AgeCheck));
    assert!(calls
        .iter()
        .any(|c| c.register == 2 && c.kind == CallKind::FastFood));
}

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
    scan(&mut s, 1, "staff", SANDWICH);
    scan(&mut s, 1, "staff", LIGHTER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    assert_eq!(reg(&s, 1).staff.required_age, Some(20));
    assert_eq!(s.snapshot().age_rules.history.len(), 1);
}

#[test]
fn customer_cannot_take_over_staff_operated_register() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
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

#[test]
fn fast_food_flow_in_self_operated_mode() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", BREAD);
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
        json!({"type":"checkout","register":2,"side":"customer"})
    )
    .is_err());
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
        json!({"type":"check_ff","register":2,"side":"staff","line_id":ff,"handed":1}),
    );
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":2,"side":"staff"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"check_ff","register":2,"side":"staff","line_id":ff,"handed":2}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"staff"}),
    );
    confirm_point(&mut s, 2);
    assert!(run(
        &mut s,
        json!({"type":"check_ff","register":2,"side":"staff","line_id":ff,"handed":2})
    )
    .is_err());
    assert_eq!(
        reg(&s, 2).mode,
        Mode::StaffOperated,
        "badge keeps the register staff_operated"
    );
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

#[test]
fn pickup_matches_the_parcel_then_hands_it_over() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", RECEIVE);
    assert!(s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.kind == CallKind::Service));
    for code in [RECEIVE, BREAD, PARCEL] {
        assert!(run(
            &mut s,
            json!({"type":"scan","register":2,"side":"customer","code":code})
        )
        .is_err());
    }
    login_as(&mut s, 2, "ST001");
    assert_eq!(reg(&s, 2).mode, Mode::StaffOperated);
    let no = reg(&s, 2).staff.lines[0]
        .line
        .pickup
        .clone()
        .expect("お預かりの番号")
        .no;
    let amount = reg(&s, 2).staff.lines[0]
        .line
        .pickup
        .clone()
        .unwrap()
        .amount;
    assert!(amount > 0);
    assert_eq!(reg(&s, 2).customer.total, amount);
    assert!(run(&mut s, json!({"type":"pickup_handover","register":2})).is_err());
    let id = reg(&s, 2).staff.lines[0].line.id;
    assert!(run(
        &mut s,
        json!({"type":"complete_service","register":2,"line_id":id,"amount":0})
    )
    .is_err());
    assert!(!allowed(&reg(&s, 2), Side::Staff, Action::Checkout));
    let other = if no.starts_with('9') {
        "PK000000000000"
    } else {
        "PK999999999999"
    };
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"staff","code":other})
    )
    .is_err());
    assert!(reg(&s, 2).staff.lines[0]
        .line
        .pickup
        .as_ref()
        .unwrap()
        .wrong
        .is_some());
    let right = format!("PK{}", no.replace('-', ""));
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"customer","code":right})
    )
    .is_err());
    scan(&mut s, 2, "staff", &right);
    let pk = reg(&s, 2).staff.lines[0].line.pickup.clone().unwrap();
    assert!(pk.matched && pk.wrong.is_none());
    let v = reg(&s, 2);
    assert!(pk.amount >= 1_000);
    assert_eq!(v.customer.total, pk.amount);
    assert!(run(&mut s, json!({"type":"pickup_handover","register":2})).is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"staff"}),
    );
    assert!(!reg(&s, 2).staff.point_pending);
    ok(
        &mut s,
        json!({"type":"select_payment","register":2,"side":"staff","method":"cash"}),
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
    assert!(v.staff.lines.is_empty() && v.pickup_handover.is_none());
    assert!(!s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.kind == CallKind::Service && c.register == 2));
    scan(&mut s, 2, "staff", BREAD);
}

#[test]
fn pickup_can_be_decided_by_found_or_missing() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", RECEIVE);
    login_as(&mut s, 2, "ST001");
    let pk = |s: &Store| reg(s, 2).staff.lines[0].line.pickup.clone().unwrap();
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
    assert!(reg(&s, 2).staff.lines.is_empty());
    scan(&mut s, 2, "staff", RECEIVE);
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
    let id = reg(&s, 2).staff.lines[0].line.id;
    assert!(run(
        &mut s,
        json!({"type":"remove_line","register":2,"side":"staff","line_id":id})
    )
    .is_err());
    assert!(pk(&s).matched);
}

fn pending_parcel(s: &Store, r: u8) -> u32 {
    reg(s, r)
        .staff
        .lines
        .iter()
        .find(|l| matches!(l.line.kind, LineKind::Service { done: false, .. }))
        .map(|l| l.line.id)
        .expect("受付中の荷物")
}

#[test]
fn collect_parcel_is_free_and_registered_parcel_is_cancelled_by_its_slip() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", PARCEL_COLLECT);
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
    let pc = |s: &Store, i: usize| reg(s, 2).staff.lines[i].line.parcel.clone().unwrap();
    assert!(pc(&s, 0).collect && pc(&s, 0).fee == 940);
    assert_eq!(reg(&s, 2).customer.total, 0);
    ok(
        &mut s,
        json!({"type":"parcel_next","register":2,"more":true}),
    );
    let id2 = pending_parcel(&s, 2);
    ok(
        &mut s,
        json!({"type":"parcel_size","register":2,"line_id":id2,"sum_cm":58,"weight_g":1500}),
    );
    ok(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id2,"postal":"1000001"}),
    );
    assert!(pc(&s, 1).collect);
    let slip = pc(&s, 0).slip.replace('-', "");
    assert_eq!(slip.len(), 12);
    assert!(run(
        &mut s,
        json!({"type":"remove_line","register":2,"side":"staff","line_id":id})
    )
    .is_err());
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"customer","code":format!("PS{slip}")})
    )
    .is_err());
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"staff","code":"PS000000000000"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"scan","register":2,"side":"staff","code":format!("PS{slip}")}),
    );
    let v = reg(&s, 2);
    assert_eq!(v.staff.lines.len(), 1);
    assert_eq!(v.staff.lines[0].line.id, id2);
}

#[test]
fn parcel_size_is_decided_in_two_steps_and_back_steps_back() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", PARCEL);
    login_as(&mut s, 2, "ST001");
    let id = pending_parcel(&s, 2);
    let parcel = |s: &Store, id: u32| {
        reg(s, 2)
            .staff
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
    assert!(reg(&s, 2).staff.lines.is_empty());
    assert!(!reg(&s, 2).staff.parcel_ask);
    scan(&mut s, 2, "customer", PARCEL);
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
    assert_eq!(v.staff.lines.len(), 1);
    assert!(v.staff.parcel_ask);
}

#[test]
fn parcel_fee_comes_from_size_and_postal_code() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", PARCEL);
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
    assert_eq!(v.customer.total, 940);
    assert!(v.staff.parcel_ask && v.customer.parcel_ask);
    assert!(!allowed(&v, Side::Staff, Action::Checkout));
    ok(
        &mut s,
        json!({"type":"parcel_next","register":2,"more":true}),
    );
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
    assert_eq!(reg(&s, 2).customer.total, 940 + 1230 + 240);
    ok(
        &mut s,
        json!({"type":"parcel_next","register":2,"more":false}),
    );
    let v = reg(&s, 2);
    assert!(!v.staff.parcel_ask);
    assert!(allowed(&v, Side::Staff, Action::Checkout));
    scan(&mut s, 2, "staff", "PC12345678");
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"staff"}),
    );
    let v = reg(&s, 2);
    assert!(!v.staff.point_pending);
    ok(
        &mut s,
        json!({"type":"select_payment","register":2,"side":"staff","method":"cash"}),
    );
    ok(
        &mut s,
        json!({"type":"insert_cash","register":2,"denomination":5000,"count":1}),
    );
    ok(&mut s, json!({"type":"finish_insert","register":2}));
    assert_eq!(reg(&s, 2).last_receipt.map(|r| r.points), Some(0));
}

#[test]
fn parcel_skips_point_card_check() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", PARCEL);
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
        json!({"type":"parcel_next","register":2,"more":false}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"staff"}),
    );
    assert!(!reg(&s, 2).staff.point_pending);
}

#[test]
fn parcel_cannot_be_mixed_with_other_goods() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", BREAD);
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"customer","code":PARCEL})
    )
    .is_err());
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", PARCEL);
    scan(&mut s, 2, "customer", PARCEL);
    assert_eq!(reg(&s, 2).customer.lines.len(), 2);
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":2,"side":"customer"})
    )
    .is_err());
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"customer","code":BREAD})
    )
    .is_err());
    assert!(run(
        &mut s,
        json!({"type":"scan","register":2,"side":"customer","code":RECEIVE})
    )
    .is_err());
}

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

#[test]
fn store_menu_opens_only_on_the_staff_side() {
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
    assert_eq!(v.active_side, Some(Side::Staff));
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":null}),
    );
    assert!(reg(&s, 1).menu.is_none());
}

#[test]
fn calls_stay_on_originating_register() {
    let mut s = Store::new(1, 4, 2);
    ok(&mut s, json!({"type":"call_staff","register":2}));
    ok(&mut s, json!({"type":"call_staff","register":4}));
    let snap = s.snapshot();
    assert_eq!(snap.registers[1].staff.calls.len(), 1);
    assert!(snap.registers[0].staff.calls.is_empty());
    assert!(snap.registers[2].staff.calls.is_empty());
    login_as(&mut s, 4, "ST002");
    let snap = s.snapshot();
    assert_eq!(snap.calls.len(), 1);
    assert_eq!(snap.calls[0].register, 2);
}

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
    assert_eq!(s.snapshot().bells[0].rings, before);
    assert!(!s.snapshot().bells[0].buzzing);
    assert!(reg(&s, 2).lamp, "the lamp stays on until staff arrives");

    ok(
        &mut s,
        json!({"type":"set_bell_offline","bell":0,"offline":true}),
    );
    advance(&mut s, 8_000);
    let snap = s.snapshot();
    assert!(!snap.bells[0].link_ok);
    assert_eq!(
        snap.registers[0].staff.bell_down,
        vec!["据え付け端末".to_string()]
    );
}

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
    assert_eq!(reg(&s, 1).mode, Mode::StaffOperated);
}

#[test]
fn staff_and_customer_screens_are_exclusive() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SelfOperated);
    assert_eq!(v.active_side, Some(Side::Customer));
    assert!(run(&mut s, json!({"type":"pause","register":1})).is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"qr"}),
    );
    scan(&mut s, 1, "staff", "ST002");
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::StaffOperated);
    assert_eq!(v.active_side, None);
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"staff"}),
    );
    assert_eq!(reg(&s, 1).active_side, Some(Side::Staff));
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"})
    )
    .is_err());
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SelfOperated);
    assert_eq!(v.active_side, Some(Side::Customer));
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    assert!(!reg(&s, 1).customer.point_pending);
}

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

#[test]
fn mode_follows_staff_badge() {
    let mut s = Store::new(1, 2, 1);
    assert!(s
        .snapshot()
        .registers
        .iter()
        .all(|r| r.mode == Mode::SelfOperated && r.staff.state == StaffState::Absent));
    login(&mut s, 1);
    assert_eq!(reg(&s, 1).mode, Mode::StaffOperated);
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SelfOperated);
}

#[test]
fn customer_cannot_register_ff_in_self_operated() {
    let mut s = Store::new(1, 2, 1);
    assert!(run(
        &mut s,
        json!({"type":"register_ff","register":2,"side":"customer","code":"FF01","qty":1})
    )
    .is_err());
    ok(&mut s, json!({"type":"ff_button","register":2}));
}

#[test]
fn idle_register_returns_to_self_operated() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 2);
    advance(&mut s, 4 * 60_000);
    scan(&mut s, 2, "staff", BREAD);
    ok(&mut s, json!({"type":"void_transaction","register":2}));
    advance(&mut s, 4 * 60_000);
    assert_eq!(reg(&s, 2).mode, Mode::StaffOperated);
    advance(&mut s, 61_000);
    let v = reg(&s, 2);
    assert_eq!(v.mode, Mode::SelfOperated);
    assert_eq!(v.staff.state, StaffState::Absent);
    assert!(v.idle_revert_in.is_none());
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
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
    assert!(v.customer.lines.is_empty());
    assert_eq!(v.customer.inserted, 0);
    assert_eq!(v.outlet, 100);
    scan(&mut s, 2, "customer", BREAD);
    assert!(reg(&s, 2).idle_voids);
    advance(&mut s, 5 * 60_000 + 1_000);
    assert!(reg(&s, 2).customer.lines.is_empty());
    assert!(reg(&s, 2).idle_revert_in.is_none());
}

#[test]
fn cash_in_and_out_from_the_customer_side_menu() {
    let mut s = Store::new(1, 2, 1);
    let cin = json!({"type":"cash_in","register":1,"kind":"deposit","amount":3000});
    assert!(run(&mut s, cin.clone()).is_err());
    login(&mut s, 1);
    assert!(
        run(&mut s, cin.clone()).is_err(),
        "店舗メニューの外では入金できない"
    );
    open_staff_menu(&mut s, 1);
    let before = reg(&s, 1).staff.drawer_total;
    ok(&mut s, cin);
    ok(
        &mut s,
        json!({"type":"cash_in","register":1,"kind":"changer_refill","amount":1500}),
    );
    assert_eq!(reg(&s, 1).staff.drawer_total, before + 4500);
    ok(
        &mut s,
        json!({"type":"cash_out","register":1,"kind":"changer_collect","amount":4500,"password":"7777"}),
    );
    assert_eq!(reg(&s, 1).staff.drawer_total, before);
    let ops: Vec<_> = s.snapshot().audit.iter().map(|a| a.operation).collect();
    assert_eq!(ops, vec!["取引外入金", "釣銭補充", "釣銭機回収"]);
    scan(&mut s, 1, "staff", BREAD);
    assert!(run(
        &mut s,
        json!({"type":"cash_in","register":1,"kind":"deposit","amount":100})
    )
    .is_err());
}

#[test]
fn inlet_pays_and_change_waits_in_outlet() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":1000})).is_err());
    login(&mut s, 1);
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":1000})).is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    ok(&mut s, json!({"type":"insert","register":1,"amount":100}));
    assert!(
        run(&mut s, json!({"type":"finish_insert","register":1})).is_err(),
        "not enough yet"
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    let v = reg(&s, 1);
    assert_eq!(v.staff.inserted, 1100);
    assert_eq!(
        v.staff.method,
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

#[test]
fn store_menu_uses_inlet_and_outlet() {
    let mut s = Store::new(1, 2, 1);
    let open =
        |screen: &str| json!({"type":"open_menu","register":1,"side":"staff","screen":screen});
    assert!(
        run(&mut s, open("top")).is_err(),
        "only for logged-in staff"
    );
    login(&mut s, 1);
    open_staff_menu(&mut s, 1);
    let v = reg(&s, 1);
    assert_eq!(v.active_side, Some(Side::Staff));
    ok(&mut s, open("deposit"));
    let before = reg(&s, 1).staff.drawer_total;
    ok(&mut s, json!({"type":"insert","register":1,"amount":2000}));
    assert_eq!(reg(&s, 1).staff.drawer_total, before + 2000);
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"side":"staff","screen":"changer","exchange_to":1000}),
    );
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":1500})).is_err());
    ok(&mut s, json!({"type":"insert","register":1,"amount":10000}));
    let v = reg(&s, 1);
    assert_eq!(v.outlet_detail, vec![(1000, 10)]);
    assert_eq!(v.staff.drawer_total, before + 2000);
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    ok(&mut s, open("emergency"));
    ok(
        &mut s,
        json!({"type":"cash_out","register":1,"kind":"emergency","amount":5000,"password":"7777"}),
    );
    assert_eq!(reg(&s, 1).outlet, 5000);
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"side":"staff","screen":null}),
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
        json!({"type":"scan","register":1,"side":"staff","code":"ST002"}),
    );
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert!(reg(&s, 1).menu.is_none());
}

#[test]
fn staff_operated_screens_lock_only_while_touched() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "staff", "ST001");
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::StaffOperated);
    assert_eq!(v.active_side, None);
    assert!(allowed(&v, Side::Customer, Action::RegisterFf));
    assert!(allowed(&v, Side::Staff, Action::RegisterFf));
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"customer","code":"FF01","qty":1}),
    );
    assert!(reg(&s, 1).staff.calls.is_empty());
    assert_eq!(reg(&s, 1).active_side, Some(Side::Customer));
    let e = run(&mut s, json!({"type":"claim","register":1,"side":"staff"})).unwrap_err();
    assert!(e.contains("操作中"), "{e}");
    assert!(run(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"staff","code":"FF02","qty":1})
    )
    .is_err());
    advance(&mut s, 1_000);
    assert_eq!(reg(&s, 1).active_side, None);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"staff","code":"FF02","qty":1}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.staff.lines.len(), 2);
    assert!(!allowed(&v, Side::Staff, Action::Checkout));
    assert!(allowed(&v, Side::Staff, Action::CheckFf));
    let ids: Vec<u32> = v.staff.lines.iter().map(|l| l.line.id).collect();
    let check = |id: u32, side: &str| json!({"type":"check_ff","register":1,"side":side,"line_id":id,"handed":1});
    assert!(run(&mut s, check(ids[0], "customer")).is_err());
    ok(&mut s, check(ids[0], "staff"));
    assert!(!allowed(&reg(&s, 1), Side::Staff, Action::Checkout));
    advance(&mut s, 1_000);
    assert!(run(&mut s, check(ids[1], "customer")).is_err());
    ok(&mut s, check(ids[1], "staff"));
    advance(&mut s, 1_000);
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
}

#[test]
fn called_register_lamp_lights_at_once_until_staff_arrives() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
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
    scan(&mut s, 1, "staff", "ST001");
    assert!(!reg(&s, 1).lamp, "staff arrived");
    assert!(reg(&s, 2).lamp);
}

#[test]
fn wallet_card_must_match_chosen_method() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
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
    advance(&mut s, 20_000);
    scan(&mut s, 2, "customer", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"customer"}),
    );
    let (r1, r2) = (reg(&s, 1), reg(&s, 2));
    assert!(
        r1.ring_volume > r2.ring_volume,
        "the first caller is louder"
    );
    assert_eq!(r1.call_waiting_ms, Some(20_000));
    scan(&mut s, 1, "staff", "ST001");
    assert_eq!(reg(&s, 1).ring_volume, 0.0);
}

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
    ok(&mut s, json!({"type":"release_call","register":1}));
    advance(&mut s, 60_000);
    assert_eq!(kinds(&s, 1).len(), 3);
    scan(&mut s, 2, "customer", PARCEL);
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
        reg(&s, 1).staff.calls.is_empty(),
        "no call record, so no call screen"
    );
    ok(&mut s, json!({"type":"release_call","register":1}));
}

#[test]
fn call_button_rings_only_while_held_and_every_press_rings() {
    for seed in 1..20u64 {
        let mut s = Store::new(seed, 2, 1);
        ok(&mut s, json!({"type":"set_bell_loss","percent":70}));
        advance(&mut s, 3_000);
        for (i, topic) in [json!(null), json!("tobacco"), json!("service")]
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
            advance(&mut s, 30_000);
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

#[test]
fn logout_returns_to_self_operated_even_while_customer_screen_holds() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "staff", "ST001");
    assert_eq!(reg(&s, 1).mode, Mode::StaffOperated);
    assert_eq!(reg(&s, 1).active_side, None);
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SelfOperated);
    scan(&mut s, 1, "staff", "ST001");
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"customer","code":"FF01","qty":1}),
    );
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SelfOperated);
}

#[test]
fn checkout_from_held_customer_screen_then_point_card_check() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "staff", "ST001");
    assert_eq!(reg(&s, 1).active_side, None);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"customer","code":"FF01","qty":1}),
    );
    let id = reg(&s, 1).customer.lines[0].id;
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"staff","line_id":id,"handed":1}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.customer.phase, Phase::Paying);
    assert!(v.customer.point_pending);
    assert!(run(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"cash"})
    )
    .is_err());
    scan(&mut s, 1, "customer", "PC12345678");
    let v = reg(&s, 1);
    assert_eq!(v.customer.phase, Phase::Paying);
    assert!(!v.customer.point_pending && v.customer.point_card);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"cash"}),
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":500}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.unwrap().total, 220);

    scan(&mut s, 1, "customer", "PC12345678");
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"customer","code":"FF03","qty":1}),
    );
    let id = reg(&s, 1).customer.lines[0].id;
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"staff","line_id":id,"handed":1}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    assert!(!reg(&s, 1).customer.point_pending);
}

#[test]
fn either_screen_can_operate_except_age_confirmation() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "staff", "ST001");
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
    let v = reg(&s, 1);
    assert!(v.staff.age_choice_pending);
    assert!(run(
        &mut s,
        json!({"type":"choose_age_check","register":1,"side":"customer","id_check":false})
    )
    .is_err());
    no_id_check(&mut s, 1);
    ok(&mut s, json!({"type":"void_transaction","register":1}));
}

#[test]
fn staff_skipping_the_age_check_needs_no_customer_button() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    no_id_check(&mut s, 1);
    let v = reg(&s, 1);
    assert!(!v.staff.age_choice_pending && !v.staff.id_pending);
    assert!(v.staff.point_pending);
    confirm_point(&mut s, 1);
    assert_eq!(reg(&s, 1).staff.payment_blocker, None);
}

fn check_bill(s: &mut Store, r: u8, line_id: u32, checked: bool) {
    ok(
        s,
        json!({"type":"check_bill","register":r,"side":"staff","line_id":line_id,"checked":checked}),
    );
}

fn affix_stamp(s: &mut Store, r: u8, line_id: u32, affixed: bool) {
    ok(
        s,
        json!({"type":"affix_stamp","register":r,"side":"staff","line_id":line_id,"affixed":affixed}),
    );
}

#[test]
fn revenue_stamp_can_be_confirmed_while_registering_and_blocks_payment_until_then() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BILL);
    let id = bill_ids(&s, 1)[0];
    affix_stamp(&mut s, 1, id, true);
    affix_stamp(&mut s, 1, id, false);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    check_bill(&mut s, 1, id, true);
    assert!(reg(&s, 1)
        .staff
        .payment_blocker
        .unwrap()
        .contains("収入印紙"));
    affix_stamp(&mut s, 1, id, true);
    assert_eq!(reg(&s, 1).staff.payment_blocker, None);
    ok(&mut s, json!({"type":"insert","register":1,"amount":50000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).bill_handover, Some(1));
}

#[test]
fn a_tax_slip_of_the_same_amount_needs_no_revenue_stamp() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", TAX_BILL);
    let v = reg(&s, 1);
    assert_eq!(v.staff.bills[0].amount, 50_000);
    assert_eq!(v.staff.bills[0].stamp, 0);
    assert!(!v.staff.stamp_pending);
    let id = bill_ids(&s, 1)[0];
    assert!(run(
        &mut s,
        json!({"type":"affix_stamp","register":1,"line_id":id,"affixed":true})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    check_bill(&mut s, 1, id, true);
    assert_eq!(reg(&s, 1).staff.payment_blocker, None);
    ok(&mut s, json!({"type":"insert","register":1,"amount":50000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).bill_handover, Some(1));
}

/// たくさんの買い物で合計 56,200円（すべて 10%、税抜 51,091円）：たばこカートン 6,000円×9、化粧品 2,200円×1
fn big_basket(s: &mut Store, side: &str) {
    for (code, qty) in [("4900000000158", 9), ("4910000000069", 1)] {
        scan(s, 1, side, code);
        let id = reg(s, 1)
            .staff
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
    assert_eq!(reg(s, 1).staff.total, 56_200);
}

#[test]
fn a_cash_purchase_of_50000_yen_before_tax_needs_a_stamp_on_the_receipt() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    big_basket(&mut s, "staff");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
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
        json!({"type":"affix_receipt_stamp","register":1,"side":"customer"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"affix_receipt_stamp","register":1,"side":"staff"}),
    );
    assert_eq!(reg(&s, 1).receipt_stamp, None);
    advance(&mut s, 10_000);
    assert!(reg(&s, 1).last_receipt.is_none());
}

#[test]
fn a_self_operated_purchase_needing_a_stamp_calls_staff() {
    let mut s = Store::new(1, 2, 1);
    // 化粧品 2,200円×25＝55,000円（税抜 50,000円）
    scan(&mut s, 1, "customer", "4910000000069");
    let id = reg(&s, 1).customer.lines[0].id;
    ok(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"customer","line_id":id,"qty":25}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"qr"}),
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
        json!({"type":"affix_receipt_stamp","register":1,"side":"staff"})
    )
    .is_err());
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"affix_receipt_stamp","register":1,"side":"staff"}),
    );
    assert!(s.snapshot().calls.is_empty());
}

#[test]
fn a_receipt_under_50000_yen_before_tax_or_paid_by_credit_needs_no_stamp() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    big_basket(&mut s, "staff");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    no_id_check(&mut s, 1);
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"credit"}),
    );
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"credit"}),
    );
    assert_eq!(reg(&s, 1).receipt_stamp, None);
}

#[test]
fn revenue_stamp_waits_for_staff_in_self_operated_mode() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BILL);
    let id = bill_ids(&s, 1)[0];
    assert!(run(
        &mut s,
        json!({"type":"affix_stamp","register":1,"side":"staff","line_id":id,"affixed":true})
    )
    .is_err());
    assert!(reg(&s, 1).customer.stamp_pending);
}

fn bill_ids(s: &Store, r: u8) -> Vec<u32> {
    reg(s, r).staff.bills.iter().map(|b| b.line_id).collect()
}

#[test]
fn utility_bills_are_checked_by_staff_after_checkout_and_paid_in_cash() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    for code in [BILL, WATER_BILL, GAS_BILL, BILL] {
        scan(&mut s, 1, "staff", code);
    }
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    let v = reg(&s, 1);
    assert!(!v.staff.point_pending && !v.staff.age_choice_pending);
    assert_eq!(v.staff.method, Some(PaymentMethod::Cash));
    assert!(v.staff.bill_unchecked);
    let rows: Vec<_> = v.staff.bills.iter().map(|b| (b.name, b.amount)).collect();
    assert_eq!(
        rows,
        [
            ("電気料金", 50_000),
            ("水道料金", 4_000),
            ("ガス料金", 3_000),
            ("電気料金", 50_000)
        ]
    );
    let nos: Vec<_> = v.staff.bills.iter().map(|b| b.slip_no.clone()).collect();
    for no in &nos {
        assert_eq!(no.len(), 19, "{no}");
        assert!(no
            .split(' ')
            .all(|g| g.len() == 4 && g.bytes().all(|c| c.is_ascii_digit())));
    }
    assert_ne!(nos[0], nos[3]);
    let ids = bill_ids(&s, 1);
    assert!(v.staff.payment_blocker.unwrap().contains("公共料金の内容"));
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":20000})).is_err());
    let e = run(
        &mut s,
        json!({"type":"check_bill","register":1,"side":"customer","line_id":ids[0],"checked":true}),
    )
    .unwrap_err();
    assert!(e.contains("スタッフ側"), "{e}");
    check_bill(&mut s, 1, ids[0], true);
    let v = reg(&s, 1);
    assert!(v.staff.bills[0].checked);
    assert!(
        !v.staff.bills[3].checked,
        "同じ電気料金でも一枚ずつ確認する"
    );
    check_bill(&mut s, 1, ids[0], false);
    assert!(!reg(&s, 1).staff.bills[0].checked);
    for &id in &ids[..3] {
        check_bill(&mut s, 1, id, true);
    }
    assert!(reg(&s, 1).staff.bill_unchecked);
    check_bill(&mut s, 1, ids[3], true);
    let v = reg(&s, 1);
    assert!(!v.staff.bill_unchecked);
    let stamps: Vec<_> = v.staff.bills.iter().map(|b| b.stamp).collect();
    assert_eq!(
        stamps,
        [200, 0, 0, 200],
        "5万円以上の払込票だけ収入印紙が要る"
    );
    assert!(v.staff.stamp_pending);
    assert!(v.staff.payment_blocker.unwrap().contains("収入印紙"));
    assert!(run(
        &mut s,
        json!({"type":"insert","register":1,"amount":200000})
    )
    .is_err());
    let e = run(
        &mut s,
        json!({"type":"affix_stamp","register":1,"side":"customer","line_id":ids[0],"affixed":true}),
    )
    .unwrap_err();
    assert!(e.contains("スタッフ側"), "{e}");
    assert!(
        run(
            &mut s,
            json!({"type":"affix_stamp","register":1,"line_id":ids[1],"affixed":true})
        )
        .is_err(),
        "印紙の要らない払込票は確認しない"
    );
    affix_stamp(&mut s, 1, ids[0], true);
    assert!(reg(&s, 1).staff.stamp_pending, "一枚ずつ確認する");
    affix_stamp(&mut s, 1, ids[3], true);
    let v = reg(&s, 1);
    assert!(!v.staff.stamp_pending);
    assert_eq!(v.staff.payment_blocker, None);
    for m in ["credit", "qr", "points"] {
        assert!(
            run(
                &mut s,
                json!({"type":"select_payment","register":1,"side":"staff","method":m})
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
    assert_eq!((r.total, r.method, r.points), (107_000, "現金", 0));
    assert_eq!(v.bill_handover, Some(4));
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    advance(&mut s, 10_000);
    assert!(reg(&s, 1).last_receipt.is_some());
    assert!(run(
        &mut s,
        json!({"type":"hand_over_bills","register":1,"side":"customer"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"hand_over_bills","register":1,"side":"staff"}),
    );
    assert_eq!(reg(&s, 1).bill_handover, None);
    advance(&mut s, 100);
    assert!(reg(&s, 1).last_receipt.is_none());
}

#[test]
fn self_operated_bill_checkout_calls_staff_to_check_the_slips() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", GAS_BILL);
    scan(&mut s, 1, "customer", GAS_BILL);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    let calls = s.snapshot().calls;
    assert!(calls
        .iter()
        .any(|c| c.register == 1 && c.kind == CallKind::Bill && c.detail == "2枚"));
    let v = reg(&s, 1);
    assert!(v.customer.bill_unchecked && v.customer.staff_called);
    assert!(!v.customer.point_pending);
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":10000})).is_err());
    let id = bill_ids(&s, 1)[0];
    assert!(run(
        &mut s,
        json!({"type":"check_bill","register":1,"side":"staff","line_id":id,"checked":true})
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
    scan(&mut s, 1, "staff", BREAD);
    assert_eq!(reg(&s, 1).bill_handover, None);
}

#[test]
fn utility_bills_cannot_share_a_transaction_with_other_goods() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"staff","code":BILL}),
    )
    .unwrap_err();
    assert!(e.contains("一緒に登録できません"), "{e}");
    ok(
        &mut s,
        json!({"type":"void_transaction","register":1,"side":"staff"}),
    );
    scan(&mut s, 1, "staff", BILL);
    for cmd in [
        json!({"type":"scan","register":1,"side":"staff","code":BREAD}),
        json!({"type":"register_ff","register":1,"code":"FF01","qty":1}),
    ] {
        let e = run(&mut s, cmd).unwrap_err();
        assert!(e.contains("一緒に登録できません"), "{e}");
    }
    assert_eq!(reg(&s, 1).staff.lines.len(), 1);
}

#[test]
fn registered_bill_slips_cannot_be_removed_only_voided() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BILL);
    scan(&mut s, 1, "customer", WATER_BILL);
    let id = reg(&s, 1).staff.lines[0].line.id;
    let try_remove = |s: &mut Store, side: &str| {
        let e = run(
            s,
            json!({"type":"remove_line","register":1,"side":side,"line_id":id}),
        )
        .unwrap_err();
        assert!(e.contains("取引中止"), "{e}");
    };
    try_remove(&mut s, "customer");
    login(&mut s, 1);
    try_remove(&mut s, "staff");
    assert_eq!(reg(&s, 1).staff.lines.len(), 2);
    ok(
        &mut s,
        json!({"type":"void_transaction","register":1,"side":"staff"}),
    );
    assert!(reg(&s, 1).staff.lines.is_empty());
}

#[test]
fn points_pay_only_for_point_eligible_goods() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", "PC12345678");
    scan(&mut s, 1, "staff", TOBACCO);
    scan(&mut s, 1, "staff", SANDWICH);
    let price = |s: &Store, code: &str| {
        reg(s, 1)
            .staff
            .lines
            .iter()
            .find(|l| l.line.code == code)
            .map(|l| l.amount)
            .unwrap()
    };
    let (tob, sand) = (price(&s, TOBACCO), price(&s, SANDWICH));
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    no_id_check(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"points"}),
    );
    use_points(&mut s, 1, "all", None);
    let v = reg(&s, 1);
    assert_eq!(v.prepaid, sand);
    assert_eq!(v.staff.due, tob);
    assert!(run(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"points"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"credit"}),
    );
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"credit"}),
    );
    let r = reg(&s, 1).last_receipt.expect("決済済み");
    assert_eq!(r.method, "ポイント利用・クレジットカード");
    assert_eq!(r.points_used, sand);
    assert_eq!(r.points, 0, "no points on the part paid by points or on tobacco");
    assert_eq!(r.point_balance, Some(1_250 - sand));

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", "PC12345678");
    scan(&mut s, 1, "staff", TOBACCO);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    no_id_check(&mut s, 1);
    assert!(reg(&s, 1)
        .unavailable
        .iter()
        .any(|u| u.method == PaymentMethod::Points));
}

#[test]
fn self_operated_cash_choice_calls_staff() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"cash"}),
    );
    assert!(reg(&s, 1).staff.calls.iter().any(|c| c.label == "現金"));
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":1000})).is_err());
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"emoney"}),
    );
    assert!(reg(&s, 1).staff.calls.is_empty());
}

#[test]
fn self_operated_back_from_cash_choice_withdraws_the_call() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"cash"}),
    );
    assert!(reg(&s, 1).staff.calls.iter().any(|c| c.label == "現金"));
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"customer"}),
    );
    assert!(reg(&s, 1).staff.calls.is_empty());
}

#[test]
fn stamps_are_paid_in_cash_even_with_other_items() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    scan(&mut s, 1, "customer", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    let v = reg(&s, 1);
    assert!(v.staff.calls.iter().any(|c| c.label == "現金"));
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"emoney"}),
    );
    assert!(run(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"})
    )
    .is_err());
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":500})).is_err());
    login(&mut s, 1);
    assert!(reg(&s, 1).staff.calls.is_empty());

    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    for m in ["emoney", "qr", "credit", "points"] {
        assert!(
            run(
                &mut s,
                json!({"type":"select_payment","register":1,"side":"customer","method":m})
            )
            .is_err(),
            "{m}"
        );
    }
    assert_eq!(
        reg(&s, 1).staff.taxes.iter().map(|t| t.tax).sum::<u32>(),
        0,
        "非課税"
    );

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "customer", BREAD);
    scan(&mut s, 1, "customer", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"emoney"}),
    );
    assert_eq!(reg(&s, 1).cashless_due, 160);
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    let v = reg(&s, 1);
    assert_eq!(emoney_balance(&s), 840);
    assert_eq!(v.customer.due, 110);
    ok(&mut s, json!({"type":"insert","register":1,"amount":500}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.outlet, 390);
    assert_eq!(v.last_receipt.unwrap().method, "電子マネー・現金");

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "customer", BREAD);
    scan(&mut s, 1, "customer", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"emoney"}),
    );
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    assert_eq!(emoney_balance(&s), 1_000);
}

#[test]
fn shopping_is_limited_to_100_items() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", SNACK);
    let id = reg(&s, 1).customer.lines[0].id;
    ok(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"customer","line_id":id,"qty":99}),
    );
    login(&mut s, 1);
    advance(&mut s, 1_000);
    let ff = |qty: u32| json!({"type":"register_ff","register":1,"side":"staff","code":"FF01","qty":qty});
    let e = run(&mut s, ff(2)).unwrap_err();
    assert!(e.contains("100点"), "{e}");
    ok(&mut s, ff(1));
    let v = reg(&s, 1);
    assert_eq!(v.staff.lines.iter().map(|l| l.line.qty).sum::<u32>(), 100);
    assert!(!allowed(&v, Side::Staff, Action::RegisterFf));
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"customer","code":SNACK}),
    )
    .unwrap_err();
    assert!(e.contains("100点"), "{e}");
    let ffid = reg(&s, 1)
        .staff
        .lines
        .iter()
        .map(|l| l.line.id)
        .max()
        .unwrap();
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"staff","line_id":ffid}),
    );
    scan(&mut s, 1, "customer", SNACK);
}

#[test]
fn store_menu_cannot_open_during_a_transaction() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "customer", BREAD);
    let top = json!({"type":"open_menu","register":1,"screen":"top"});
    assert!(run(&mut s, top.clone()).is_err(), "取引中は開けない");
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    assert!(run(&mut s, top.clone()).is_err(), "会計中も開けない");
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    assert!(reg(&s, 1).menu.is_none());
    ok(&mut s, top);
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SelfOperated);
}

#[test]
fn staff_enters_the_birth_date_and_unsellable_items_are_confiscated() {
    let mut s = Store::new(1, 2, 1);
    ok(
        &mut s,
        json!({"type":"set_product_age","owner_id":"000","password":"9999","code":LIGHTER,"min_age":18}),
    );
    login(&mut s, 1);
    for code in [BREAD, BEER, BEER, LIGHTER] {
        scan(&mut s, 1, "staff", code);
    }
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    advance(&mut s, 1_000);
    let v = reg(&s, 1);
    assert!(v.staff.age_choice_pending);
    let birth = |b: &str| json!({"type":"check_id","register":1,"birth":b,"today":"2026-10-19"});
    assert!(run(&mut s, birth("20070501")).is_err());
    assert!(run(
        &mut s,
        json!({"type":"choose_age_check","register":1,"side":"customer","id_check":true})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"choose_age_check","register":1,"id_check":true}),
    );
    let v = reg(&s, 1);
    assert!(v.staff.id_pending && !v.staff.age_choice_pending);
    let cash = json!({"type":"insert_cash","register":1,"denomination":1000,"count":1});
    assert!(run(&mut s, cash.clone()).is_err());
    assert!(run(&mut s, birth("20270101")).is_err());
    let msg = run(&mut s, birth("20070501")).unwrap().unwrap();
    assert!(msg.contains("19歳") && msg.contains("アルコール"), "{msg}");
    let v = reg(&s, 1);
    let codes: Vec<&str> = v.staff.lines.iter().map(|l| l.line.code).collect();
    assert!(!codes.contains(&BEER) && codes.contains(&LIGHTER) && codes.contains(&BREAD));
    assert_eq!(v.staff.confiscated.len(), 1);
    assert_eq!(v.staff.confiscated[0].qty, 2);
    assert!(!v.staff.id_pending && v.staff.phase == Phase::Paying);
    ok(&mut s, json!({"type":"dismiss_confiscated","register":1}));
    assert!(reg(&s, 1).staff.confiscated.is_empty());
    advance(&mut s, 1_000);
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"cash"}),
    );
    ok(&mut s, cash);
}

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

#[test]
fn coupons_apply_whenever_the_target_is_registered() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", "CP0001");
    let v = reg(&s, 1);
    assert_eq!(v.customer.coupons.len(), 1);
    assert_eq!(v.customer.coupons[0].discount, None);
    assert_eq!(v.customer.total, 0);
    scan(&mut s, 1, "customer", TEA);
    let v = reg(&s, 1);
    assert_eq!(v.customer.coupons[0].discount, Some(150));
    assert_eq!(v.customer.total, 0);
    scan(&mut s, 1, "customer", "CP0001");
    assert_eq!(reg(&s, 1).customer.coupons[1].discount, None);
    scan(&mut s, 1, "customer", TEA);
    let v = reg(&s, 1);
    assert_eq!(v.customer.coupons[1].discount, Some(150));
    assert_eq!(v.customer.total, 0);
    scan(&mut s, 1, "customer", SANDWICH);
    scan(&mut s, 1, "customer", TEA);
    scan(&mut s, 1, "customer", "CP0002");
    let v = reg(&s, 1);
    assert_eq!(v.customer.total, 150 + 320 - 50);
    assert_eq!(v.staff.lines.iter().map(|l| l.line.qty).sum::<u32>(), 4);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"emoney"}),
    );
    ok(&mut s, json!({"type":"tap_emoney","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.unwrap().total, 420);
}

#[test]
fn a_coupon_without_its_target_blocks_payment_until_it_is_returned() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", "CP0001");
    scan(&mut s, 1, "customer", "4900000000028");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    let v = reg(&s, 1);
    assert!(v.staff.coupon_unmatched && v.customer.coupon_unmatched);
    assert!(v
        .customer
        .payment_blocker
        .unwrap()
        .contains("適用先のないクーポン"));
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"emoney"}),
    );
    assert!(run(&mut s, json!({"type":"tap_emoney","register":1})).is_err());
    let id = reg(&s, 1).customer.coupons[0].id;
    ok(
        &mut s,
        json!({"type":"remove_coupon","register":1,"side":"customer","coupon_id":id}),
    );
    let v = reg(&s, 1);
    assert!(v.customer.coupons.is_empty() && !v.customer.coupon_unmatched);
    assert_eq!(v.staff.returned.len(), 1);
    assert!(v.customer.returned[0].unmatched);
    ok(&mut s, json!({"type":"dismiss_returned","register":1}));
    assert!(reg(&s, 1).staff.returned.is_empty());
    ok(&mut s, json!({"type":"tap_emoney","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.unwrap().total, 320);
}

#[test]
fn same_item_merges_only_when_scanned_in_a_row() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", SNACK);
    scan(&mut s, 1, "customer", SNACK);
    scan(&mut s, 1, "customer", TEA);
    scan(&mut s, 1, "customer", SNACK);
    scan(&mut s, 1, "customer", SNACK);
    scan(&mut s, 1, "customer", SNACK);
    let v = reg(&s, 1);
    let mut lines: Vec<_> = v.customer.lines.iter().collect();
    lines.sort_by_key(|l| std::cmp::Reverse(l.touched));
    let rows: Vec<_> = lines.iter().map(|l| (l.name, l.qty)).collect();
    assert_eq!(rows, [("袋菓子", 3), ("ソフトドリンク", 1), ("袋菓子", 2)]);
}

#[test]
fn staff_or_customer_changes_a_line_quantity() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", SNACK);
    scan(&mut s, 1, "customer", SANDWICH);
    let id = reg(&s, 1).customer.lines[0].id;
    let set =
        |qty: u32| json!({"type":"set_qty","register":1,"side":"customer","line_id":id,"qty":qty});
    ok(&mut s, set(3));
    let v = reg(&s, 1);
    let top = v.customer.lines.iter().max_by_key(|l| l.touched).unwrap();
    assert_eq!(top.name, "サンドイッチ", "the changed line keeps its place");
    let bread = v.customer.lines.iter().find(|l| l.id == id).unwrap();
    assert_eq!(bread.qty, 3);
    assert_eq!(v.customer.total, 150 * 3 + 320);
    ok(&mut s, set(99));
    assert!(run(&mut s, set(100)).is_err(), "over the 100-item limit");
    assert!(run(&mut s, set(0)).is_err(), "cancel is a separate button");
    ok(&mut s, set(1));
    assert_eq!(reg(&s, 1).customer.total, 150 + 320);
}

#[test]
fn coupon_and_line_ids_never_collide() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    scan(&mut s, 1, "customer", "4900000000028");
    scan(&mut s, 1, "customer", "CP0002");
    scan(&mut s, 1, "customer", "4900000000011");
    let v = reg(&s, 1);
    let mut ids: Vec<u32> = v.customer.lines.iter().map(|l| l.id).collect();
    ids.extend(v.customer.coupons.iter().map(|c| c.id));
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n);
}

#[test]
fn points_can_pay_once_staff_chooses_them() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    scan(&mut s, 1, "staff", "PC12345678");
    assert!(run(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"points"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"points"}),
    );
    use_points(&mut s, 1, "all", None);
    assert!(reg(&s, 1).staff.lines.is_empty());
}

#[test]
fn points_payment_without_a_card_goes_back_to_the_point_card_check() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    let points = json!({"type":"select_payment","register":1,"side":"staff","method":"points"});
    ok(&mut s, points.clone());
    let v = reg(&s, 1);
    assert!(v.staff.point_pending && v.customer.point_pending);
    assert_eq!(v.staff.method, Some(PaymentMethod::Points));
    scan(&mut s, 1, "staff", "PC12345678");
    let v = reg(&s, 1);
    assert_eq!(v.staff.phase, Phase::Paying, "reading the card keeps the payment screen");
    assert_eq!(v.staff.method, Some(PaymentMethod::Points));
    use_points(&mut s, 1, "all", None);
    let v = reg(&s, 1);
    assert!(v.staff.lines.is_empty());
    assert_eq!(v.last_receipt.map(|r| r.method), Some("ポイント利用"));

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    ok(&mut s, points);
    confirm_point(&mut s, 1);
    let v = reg(&s, 1);
    assert!(!v.staff.point_pending);
    assert_eq!(v.staff.method, None);
}

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
    assert!(reg(&s, 1).staff.bell_down.is_empty());
    ok(&mut s, json!({"type":"set_bell_on","bell":2,"on":true}));
    advance(&mut s, 1_000);
    assert!(s.snapshot().bells[2].buzzing);
}

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
    scan(&mut s, 1, "staff", BREAD);
    assert!(reg(&s, 1).tobacco_asked);
    scan(&mut s, 1, "staff", "4900000000127");
    assert!(!reg(&s, 1).tobacco_asked);
    login(&mut s, 2);
    assert!(!reg(&s, 2).tobacco_asked);
}

#[test]
fn change_left_in_the_outlet_reminds_then_calls_staff() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"cash"}),
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
    assert!(v.staff.calls.is_empty());
    assert_eq!(v.idle_revert_in, Some(frozen), "countdown is paused");
    advance(&mut s, 30_000);
    let v = reg(&s, 1);
    assert_eq!(
        v.staff.calls.len(),
        1,
        "staff_operated, 1 minute: call staff"
    );
    assert_eq!(v.staff.calls[0].label, "釣銭忘れ");
    assert!(
        reg(&s, 1).last_receipt.is_some(),
        "stays on the thank-you screen while change is left"
    );
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    let v = reg(&s, 1);
    assert!(!v.change_left && v.staff.calls.is_empty());
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
    advance(&mut s, 10_000);
    assert!(
        reg(&s, 1).idle_revert_in.unwrap() < frozen,
        "countdown resumes"
    );
}

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
    scan(&mut s, 1, "customer", TEA);
    scan(&mut s, 1, "customer", TEA);
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"customer","code":TEA}),
    )
    .unwrap_err();
    assert!(e.contains("2点まで"), "{e}");
    scan(&mut s, 1, "customer", BREAD);
    let id = reg(&s, 1)
        .customer
        .lines
        .iter()
        .find(|l| l.name == item.name)
        .unwrap()
        .id;
    let set =
        |qty: u32| json!({"type":"set_qty","register":1,"side":"customer","line_id":id,"qty":qty});
    assert!(run(&mut s, set(3)).is_err());
    ok(&mut s, set(1));
    ok(&mut s, set(2));
    ok(&mut s, limit("9999", None));
    scan(&mut s, 1, "customer", TEA);
    assert!(s
        .snapshot()
        .age_rules
        .history
        .iter()
        .any(|h| h.change.contains("2点まで → 制限なし")));
}

#[test]
fn tax_rates_are_data_and_owner_only() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BEER);
    let t = reg(&s, 1).staff.taxes;
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
    scan(&mut s, 1, "staff", TEA);
    scan(&mut s, 1, "staff", BEER);
    let t = reg(&s, 1).staff.taxes;
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
    assert_eq!(reg(&s, 1).staff.taxes[1].tax, 23);
    assert_eq!(s.snapshot().tax.invoice_no, "T1234567890123");
    assert_eq!(s.snapshot().age_rules.history.len(), 2);
}

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
    scan(&mut s, 1, "staff", BREAD);
    let t = reg(&s, 1).staff.taxes;
    assert!(t
        .iter()
        .any(|x| x.rate == "5%" && x.base == 160 && x.tax == 7));
}

fn emoney_balance(s: &Store) -> u32 {
    s.snapshot().emoney_balance
}

#[test]
fn self_operated_emoney_fan_calls_staff_while_shopping() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"emoney"}),
    );
    let kinds =
        |s: &Store| -> Vec<CallKind> { reg(s, 1).staff.calls.iter().map(|c| c.kind).collect() };
    assert!(kinds(&s).contains(&CallKind::Emoney));
    login(&mut s, 1);
    assert!(
        !kinds(&s).contains(&CallKind::Emoney),
        "スタッフが来たら片付く"
    );
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"staff","kind":"menu"}),
    );
}

#[test]
fn emoney_inquiry_shows_the_balance() {
    let mut s = Store::new(1, 2, 1);
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"customer","kind":"menu"}),
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
        json!({"type":"close_emoney","register":1,"side":"customer"}),
    );
    scan(&mut s, 1, "customer", BREAD);
    for kind in ["menu", "charge", "inquiry"] {
        assert!(
            run(
                &mut s,
                json!({"type":"start_emoney","register":1,"side":"customer","kind":kind})
            )
            .is_err(),
            "お買い物の途中はチャージ・残高確認できない"
        );
    }

    let mut s = Store::new(1, 2, 1);
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"customer","kind":"inquiry"}),
    );
    assert_eq!(reg(&s, 1).emoney.unwrap().balance, None);
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    assert_eq!(reg(&s, 1).emoney.unwrap().balance, Some(1_000));
    ok(
        &mut s,
        json!({"type":"close_emoney","register":1,"side":"customer"}),
    );
    assert!(reg(&s, 1).emoney.is_none());
}

#[test]
fn emoney_charge_takes_bills_up_to_the_limit() {
    let mut s = Store::new(1, 2, 1);
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"customer","kind":"inquiry"}),
    );
    assert!(reg(&s, 1).staff.calls.is_empty(), "残高照会は呼ばない");
    ok(
        &mut s,
        json!({"type":"close_emoney","register":1,"side":"customer"}),
    );
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"customer","kind":"charge"}),
    );
    let v = reg(&s, 1);
    assert!(v.emoney.is_none());
    assert!(v.staff.calls.iter().any(|c| c.label == "電子マネー"));
    login(&mut s, 1);
    assert!(reg(&s, 1).staff.calls.is_empty());
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"staff","kind":"charge"}),
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

    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"staff","kind":"charge"}),
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":2000}));
    ok(
        &mut s,
        json!({"type":"close_emoney","register":1,"side":"staff"}),
    );
    assert_eq!(reg(&s, 1).outlet, 2000);
    assert_eq!(emoney_balance(&s), 16_000);
}

fn short_checkout(s: &mut Store) {
    short_checkout_on(s, "customer");
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
        !allowed(&v, Side::Customer, Action::InsertCash),
        "選ぶまで払えない"
    );
}

#[test]
fn emoney_short_offers_three_choices() {
    let mut s = Store::new(1, 2, 1);
    short_checkout(&mut s);
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"customer","choice":"cancel"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.emoney_short, None);
    assert_eq!(v.customer.method, None);
    assert_eq!(emoney_balance(&s), 1_000);

    let mut s = Store::new(1, 2, 1);
    short_checkout(&mut s);
    assert!(run(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"customer","choice":"cash"})
    )
    .is_err());

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    short_checkout_on(&mut s, "staff");
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"staff","choice":"cash"}),
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
        json!({"type":"resolve_emoney_short","register":1,"side":"staff","choice":"cash_confirmed"}),
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
    assert_eq!(v.customer.due, 120);
    assert_eq!(v.prepaid, 1_000);
    assert_eq!(emoney_balance(&s), 0);
    assert!(
        run(
            &mut s,
            json!({"type":"back_to_registering","register":1,"side":"staff"})
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
    short_checkout_on(&mut s, "staff");
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"staff","choice":"cash"}),
    );
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"staff","choice":"cancel"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.emoney_partial, None);
    assert_eq!(v.customer.method, None);
    assert_eq!(emoney_balance(&s), 1_000);

    let mut s = Store::new(1, 2, 1);
    short_checkout(&mut s);
    assert!(
        run(
            &mut s,
            json!({"type":"resolve_emoney_short","register":1,"side":"customer","choice":"charge"})
        )
        .is_err(),
        "セミセルフはお買い物の途中にチャージしない"
    );
    assert_eq!(reg(&s, 1).emoney_short, Some(1_000));

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    short_checkout_on(&mut s, "staff");
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"staff","choice":"charge"}),
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

#[test]
fn customer_voids_and_money_comes_back() {
    let mut s = Store::new(1, 2, 1);
    assert!(
        run(&mut s, json!({"type":"void_transaction","register":1})).is_err(),
        "取引がない"
    );
    scan(&mut s, 1, "customer", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":100})).is_err());
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    let v = reg(&s, 1);
    assert!(v.customer.lines.is_empty());
    assert_eq!(v.outlet, 0);
}

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
    scan(&mut s, 1, "staff", BREAD);
    assert!(run(&mut s, json!({"type":"pause","register":1})).is_err());
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    open_staff_menu(&mut s, 1);
    ok(&mut s, json!({"type":"pause","register":1}));
    let v = reg(&s, 1);
    assert!(v.paused);
    assert_eq!(v.mode, Mode::SelfOperated);
    assert!(v.menu.is_none());
    assert!(run(
        &mut s,
        json!({"type":"scan","register":1,"side":"customer","code":BREAD})
    )
    .is_err());
    assert!(run(&mut s, json!({"type":"call_staff","register":1})).is_err());
    scan(&mut s, 1, "staff", "ST001");
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

#[test]
fn changing_handed_ff_rewinds_or_asks_to_collect() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"staff","code":"FF01","qty":2}),
    );
    let id = reg(&s, 1).staff.lines[0].line.id;
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"staff","line_id":id,"handed":2}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"staff","line_id":id,"qty":3}),
    );
    let v = reg(&s, 1);
    assert!(matches!(
        v.staff.lines[0].line.kind,
        LineKind::Ff {
            state: FfState::Waiting
        }
    ));
    assert!(!allowed(&v, Side::Staff, Action::Checkout));
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"staff","line_id":id,"handed":3}),
    );
    assert!(matches!(
        reg(&s, 1).staff.lines[0].line.kind,
        LineKind::Ff {
            state: FfState::Waiting
        }
    ));
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"staff","line_id":id,"handed":1}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"staff","line_id":id,"qty":2}),
    );
    let v = reg(&s, 1);
    assert!(matches!(
        v.staff.lines[0].line.kind,
        LineKind::Ff {
            state: FfState::Received
        }
    ));
    assert_eq!(v.staff.ff_collect.len(), 1);
    assert_eq!(v.staff.ff_collect[0].qty, 1);
    ok(&mut s, json!({"type":"dismiss_ff_collect","register":1}));
    assert!(reg(&s, 1).staff.ff_collect.is_empty());
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"staff","line_id":id}),
    );
    assert_eq!(reg(&s, 1).staff.ff_collect[0].qty, 2);
}

#[test]
fn points_are_not_earned_on_tobacco_stamps_bills_or_parcels() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", "PC12345678");
    scan(&mut s, 1, "staff", TOBACCO);
    scan(&mut s, 1, "staff", STAMP);
    scan(&mut s, 1, "staff", BREAD);
    let bread = reg(&s, 1)
        .staff
        .lines
        .iter()
        .find(|l| l.line.code == BREAD)
        .map(|l| l.amount)
        .unwrap();
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    no_id_check(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"cash"}),
    );
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":10000,"count":1}),
    );
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.map(|r| r.points), Some(bread / 200));

    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    assert!(!reg(&s, 1).staff.point_pending);
}

#[test]
fn staff_alone_is_responsible_for_ff_handover() {
    let mut s = Store::new(1, 1, 1);
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"staff","code":"FF01","qty":2}),
    );
    let id = reg(&s, 1).staff.lines[0].line.id;
    advance(&mut s, 1_000);
    assert!(!allowed(&reg(&s, 1), Side::Staff, Action::Checkout));
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"staff","line_id":id,"handed":2}),
    );
    advance(&mut s, 1_000);
    assert!(allowed(&reg(&s, 1), Side::Staff, Action::Checkout));
    ok(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"staff","line_id":id,"qty":3}),
    );
    advance(&mut s, 1_000);
    assert!(!allowed(&reg(&s, 1), Side::Staff, Action::Checkout));
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"staff","line_id":id,"handed":1}),
    );
    advance(&mut s, 1_000);
    assert!(allowed(&reg(&s, 1), Side::Staff, Action::Checkout));
}

#[test]
fn thirty_seconds_without_insert_means_done_inserting() {
    let mut s = Store::new(1, 2, 1);
    s.tick(1000);
    login(&mut s, 1);
    scan(&mut s, 1, "customer", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    assert_eq!(reg(&s, 1).staff.method, None, "method not chosen yet");
    ok(&mut s, json!({"type":"insert","register":1,"amount":100}));
    assert_eq!(
        reg(&s, 1).staff.method,
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

#[test]
fn ff_handover_cannot_be_confirmed_after_staff_logs_out() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"staff","code":"FF01","qty":1}),
    );
    let id = reg(&s, 1).staff.lines[0].line.id;
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    advance(&mut s, 2_000);
    for side in ["customer", "staff"] {
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
        reg(&s, 1).staff.lines[0].line.kind,
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

#[test]
fn saved_state_restores_every_register() {
    let mut s = Store::new(7, 2, 3);
    let set = json!({"type":"set_product_age","owner_id":"000","password":"9999","code":SANDWICH,"min_age":20});
    ok(&mut s, set);
    scan(&mut s, 2, "customer", BEER);
    scan(&mut s, 2, "customer", TEA);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"customer"}),
    );
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    scan(&mut s, 1, "staff", SANDWICH);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
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

#[test]
fn broken_or_old_saved_state_is_refused() {
    assert!(Store::load_json("{").is_err());
    let s = Store::new(1, 2, 1);
    let mut v: Value = serde_json::from_str(&s.save_json()).unwrap();
    v["schema"] = json!(0);
    assert!(Store::load_json(&v.to_string()).is_err());
}

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
    assert_eq!(reg(&s, 1).staff.lines.len(), 1);
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

#[test]
fn back_after_inserting_cash_returns_it_and_reopens_methods() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    login(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"staff"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.outlet, 1000, "unsettled cash goes to the outlet");
    assert_eq!(v.staff.inserted, 0);
    assert_eq!(v.staff.method, None, "back to choosing a method");
    assert_eq!(v.staff.phase, Phase::Paying, "stays on the payment screen");
    assert!(v.last_receipt.is_none());
    advance(&mut s, 40_000);
    assert!(reg(&s, 1).last_receipt.is_none(), "nothing settles itself");
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"emoney"}),
    );
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"staff"}),
    );
    assert_eq!(reg(&s, 1).staff.phase, Phase::Registering);
}

#[test]
fn pressing_the_chosen_payment_method_again_clears_it() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    let pick = json!({"type":"select_payment","register":1,"side":"customer","method":"emoney"});
    ok(&mut s, pick.clone());
    assert_eq!(reg(&s, 1).staff.method, Some(PaymentMethod::Emoney));
    ok(&mut s, pick);
    assert_eq!(reg(&s, 1).staff.method, None);
    let e = run(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    )
    .unwrap_err();
    assert!(!e.is_empty());
}

#[test]
fn point_card_is_asked_once_but_can_be_read_until_paid() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    assert!(reg(&s, 1).staff.point_pending);
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"qr"}),
    );
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"staff"}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    let v = reg(&s, 1);
    assert!(!v.staff.point_pending, "declined once: never asked again");
    assert!(v.staff.point_card.is_none());
    scan(&mut s, 1, "customer", "PC12345678");
    let v = reg(&s, 1);
    assert_eq!(
        v.staff.phase,
        Phase::Paying,
        "reading the card keeps the payment screen"
    );
    assert!(v.staff.point_card.is_some());
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":500,"count":1}),
    );
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.map(|r| r.points), Some(160 / 200));
}

#[test]
fn staff_can_check_age_early_and_new_age_goods_need_it_again() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BEER);
    scan(&mut s, 1, "staff", BREAD);
    assert!(allowed(&reg(&s, 1), Side::Staff, Action::EarlyAgeCheck));
    assert!(run(
        &mut s,
        json!({"type":"early_age_check","register":1,"side":"customer","start":true})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"early_age_check","register":1,"side":"staff","start":true}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.staff.phase, Phase::Registering);
    assert!(v.staff.age_choice_pending);
    assert_eq!(v.staff.required_age, Some(20));
    ok(
        &mut s,
        json!({"type":"choose_age_check","register":1,"id_check":true}),
    );
    ok(
        &mut s,
        json!({"type":"check_id","register":1,"birth":"20100101","today":"2026-11-09"}),
    );
    let v = reg(&s, 1);
    assert!(!v.staff.age_early);
    assert_eq!(v.staff.lines.len(), 1, "beer is removed early");
    assert!(!allowed(&v, Side::Staff, Action::EarlyAgeCheck));

    scan(&mut s, 1, "staff", LIGHTER);
    let v = reg(&s, 1);
    assert!(
        !v.staff
            .lines
            .iter()
            .find(|l| l.age.is_some())
            .unwrap()
            .age_checked
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    assert!(
        reg(&s, 1).staff.age_choice_pending,
        "a lighter added after the check needs the check again"
    );
}

#[test]
fn early_age_check_can_be_cancelled_and_covers_checkout() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BEER);
    ok(
        &mut s,
        json!({"type":"early_age_check","register":1,"side":"staff","start":true}),
    );
    ok(
        &mut s,
        json!({"type":"early_age_check","register":1,"side":"staff","start":false}),
    );
    assert!(!reg(&s, 1).staff.age_choice_pending);
    ok(
        &mut s,
        json!({"type":"early_age_check","register":1,"side":"staff","start":true}),
    );
    no_id_check(&mut s, 1);
    let v = reg(&s, 1);
    assert!(v.staff.lines[0].age_checked);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    assert!(
        !reg(&s, 1).staff.age_choice_pending,
        "checked early, not asked again"
    );
}

fn ring(s: &mut Store, r: u8) {
    ok(s, json!({"type":"call_staff","register":r}));
    advance(s, 200);
    ok(s, json!({"type":"release_call","register":r}));
    advance(s, 200);
}

#[test]
fn emergency_after_ten_rings_within_three_minutes() {
    let mut s = Store::new(1, 1, 1);
    advance(&mut s, 2_000);
    assert!(!reg(&s, 1).bell_dead);
    for _ in 0..9 {
        ring(&mut s, 1);
    }
    assert!(!reg(&s, 1).emergency);
    ring(&mut s, 1);
    assert!(reg(&s, 1).emergency);
    login(&mut s, 1);
    assert!(!reg(&s, 1).emergency);
}

#[test]
fn emergency_when_the_bell_is_held_for_three_minutes() {
    let mut s = Store::new(1, 1, 1);
    advance(&mut s, 2_000);
    ok(&mut s, json!({"type":"call_staff","register":1}));
    advance(&mut s, 170_000);
    assert!(!reg(&s, 1).emergency);
    advance(&mut s, 10_000);
    assert!(reg(&s, 1).emergency);
}

#[test]
fn a_short_ring_left_unanswered_is_not_an_emergency() {
    let mut s = Store::new(1, 1, 1);
    advance(&mut s, 2_000);
    ring(&mut s, 1);
    advance(&mut s, 200_000);
    assert!(!reg(&s, 1).emergency);
}

#[test]
fn emergency_also_while_the_bell_cannot_reach_anyone() {
    let mut s = Store::new(1, 1, 1);
    ok(
        &mut s,
        json!({"type":"set_bell_offline","bell":0,"offline":true}),
    );
    advance(&mut s, 20_000);
    assert!(reg(&s, 1).bell_dead);
    for _ in 0..10 {
        ring(&mut s, 1);
    }
    assert!(reg(&s, 1).emergency);
}

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
    assert!(!reg(&s, 1).emergency);
}

#[test]
fn emergency_lasts_until_staff_log_in() {
    let mut s = Store::new(1, 1, 1);
    for _ in 0..10 {
        ring(&mut s, 1);
    }
    assert!(reg(&s, 1).emergency);
    advance(&mut s, 300_000);
    assert!(reg(&s, 1).emergency);
    login(&mut s, 1);
    assert!(!reg(&s, 1).emergency);
}

#[test]
fn two_thousand_yen_bill_is_returned_to_the_outlet() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    // 二千円札そのものは受け付けず、出金ボックスへ
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":2000,"count":1}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.staff.inserted, 0);
    assert_eq!(v.outlet, 2000);
    assert_eq!(v.outlet_detail, vec![(2000, 1)]);
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    // テンキーで 2000 と打つのは千円札 2 枚とみなす
    ok(&mut s, json!({"type":"insert","register":1,"amount":2000}));
    let v = reg(&s, 1);
    assert_eq!(v.staff.inserted, 2000);
    assert_eq!(v.outlet, 0);
}

const TICKET: &str = "2100000000074";

#[test]
fn tickets_are_issued_after_payment_and_handed_over_when_the_customer_count_matches() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", TICKET);
    for code in [TICKET, BREAD] {
        let e = run(
            &mut s,
            json!({"type":"scan","register":1,"side":"staff","code":code}),
        )
        .unwrap_err();
        assert!(e.contains("一件ずつ"), "{code}: {e}");
    }
    let v = reg(&s, 1);
    assert_eq!(v.staff.lines.len(), 1, "チケットは一件ずつ");
    assert_eq!(v.staff.total, 15_600);
    assert!(v.ticket_handover.is_empty(), "支払い前は発券しない");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    assert!(
        !reg(&s, 1).staff.point_pending,
        "ポイント対象外なのでカードの確認はない"
    );
    for m in ["credit", "qr"] {
        assert!(
            run(
                &mut s,
                json!({"type":"select_payment","register":1,"side":"staff","method":m})
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
        receive_tickets(&mut s, 1, "staff", 2).is_err(),
        "受け取り枚数はお客様側で入力する"
    );
    let m = receive_tickets(&mut s, 1, "customer", 3).unwrap().unwrap();
    assert!(m.contains("一致しません"), "{m}");
    let v = reg(&s, 1);
    assert_eq!(v.tickets_received, Some(3));
    assert_eq!(v.ticket_handover.len(), 1, "枚数が合うまで終わらない");
    assert!(
        hand_tickets(&mut s, 1, "customer", 2).is_err(),
        "お渡し枚数はスタッフ側で入力する"
    );
    let m = hand_tickets(&mut s, 1, "staff", 1).unwrap().unwrap();
    assert!(m.contains("発券枚数とお渡し枚数が一致しません"), "{m}");
    let m = hand_tickets(&mut s, 1, "staff", 2).unwrap().unwrap();
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
    let m = receive_tickets(&mut s, 1, "customer", 2).unwrap().unwrap();
    assert!(m.contains("チケット 2枚をお渡ししました"), "{m}");
    let v = reg(&s, 1);
    assert!(v.ticket_handover.is_empty());
    assert_eq!(v.outlet, 16_000 - 15_600, "お渡しが済んでからお釣りを出す");
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    advance(&mut s, 10_000);
    assert!(reg(&s, 1).last_receipt.is_none());
}

#[test]
fn tickets_cannot_share_a_transaction_with_other_goods() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"staff","code":TICKET}),
    )
    .unwrap_err();
    assert!(e.contains("チケット"), "{e}");
}

#[test]
fn self_operated_ticket_checkout_calls_staff_for_cash() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", TICKET);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
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
    scan(s, r, "staff", TICKET);
    ok(s, json!({"type":"checkout","register":r,"side":"staff"}));
    ok(s, json!({"type":"insert","register":r,"amount":16000}));
    ok(s, json!({"type":"finish_insert","register":r}));
}

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
        receive_tickets(&mut s, 1, "customer", 2).is_err(),
        "発券前は受け取り枚数を入力できない"
    );
    // 発券を待っているあいだも、レジ1はお渡しが済むまで次の取引を始められない
    let e = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"staff","code":BREAD}),
    )
    .unwrap_err();
    assert!(e.contains("チケットのお渡し"), "{e}");
    pay_tickets(&mut s, 3);
    assert_eq!(reg(&s, 3).tickets_ahead, 2, "後から支払ったレジ3は列の最後");
    receive_tickets(&mut s, 2, "customer", 2).unwrap();
    assert!(
        !reg(&s, 2).ticket_handover.is_empty(),
        "スタッフの入力がまだ"
    );
    hand_tickets(&mut s, 2, "staff", 2).unwrap();
    let (r1, r2) = (reg(&s, 1), reg(&s, 2));
    assert!(r2.ticket_handover.is_empty());
    assert!(
        r1.tickets_issued && r1.tickets_ahead == 0,
        "先のお渡しが済んだので次を発券"
    );
    hand_tickets(&mut s, 1, "staff", 2).unwrap();
    receive_tickets(&mut s, 1, "customer", 2).unwrap();
    assert!(reg(&s, 1).ticket_handover.is_empty());
    scan(&mut s, 1, "staff", BREAD);
}

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

#[test]
fn fresh_food_quantity_cannot_be_raised_on_screen() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    let id = reg(&s, 1).staff.lines[0].line.id;
    let set =
        |qty: u32| json!({"type":"set_qty","register":1,"side":"staff","line_id":id,"qty":qty});
    let e = run(&mut s, set(2)).unwrap_err();
    assert!(e.contains("日配食品"), "{e}");
    assert!(reg(&s, 1).staff.lines[0].fresh);
    // 続けて読んでも数量にまとめず、一点ずつ別の行になる
    scan(&mut s, 1, "staff", BREAD);
    let v = reg(&s, 1);
    assert_eq!(v.staff.lines.len(), 2);
    assert!(v.staff.lines.iter().all(|l| l.line.qty == 1));
    // 加工食品は数量を変えられる
    scan(&mut s, 1, "staff", SNACK);
    let snack = reg(&s, 1)
        .staff
        .lines
        .iter()
        .find(|l| l.line.code == SNACK)
        .unwrap()
        .line
        .id;
    ok(
        &mut s,
        json!({"type":"set_qty","register":1,"side":"staff","line_id":snack,"qty":3}),
    );
}

#[test]
fn expired_onigiri_blocks_checkout_until_removed() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    let m = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"staff","code":ONIGIRI}),
    )
    .unwrap()
    .unwrap();
    assert!(m.contains("販売期限が切れています"), "{m}");
    let v = reg(&s, 1);
    let l = v
        .staff
        .lines
        .iter()
        .find(|l| l.line.code == ONIGIRI)
        .unwrap();
    assert!(l.expired && l.fresh);
    assert!(v.customer.lines.iter().any(|l| l.expired));
    assert!(!allowed(&v, Side::Staff, Action::Checkout));
    let id = l.line.id;
    ok(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"staff","line_id":id}),
    );
    let v = reg(&s, 1);
    assert!(allowed(&v, Side::Staff, Action::Checkout));
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
}

#[test]
fn self_operated_expired_scan_calls_staff() {
    let mut s = Store::new(1, 2, 1);
    let m = run(
        &mut s,
        json!({"type":"scan","register":1,"side":"customer","code":ONIGIRI}),
    )
    .unwrap()
    .unwrap();
    assert!(m.contains("スタッフをお呼びしました"), "{m}");
    assert!(s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.kind == CallKind::Expired));
    let id = reg(&s, 1).customer.lines[0].id;
    assert!(run(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"customer","line_id":id}),
    );
    assert!(!s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.kind == CallKind::Expired));
}

const CARTON: &str = "4900000000158";
const FIREWORKS: &str = "4910000000076";

#[test]
fn goods_needing_an_age_already_checked_do_not_redo_the_check() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", LIGHTER);
    ok(
        &mut s,
        json!({"type":"early_age_check","register":1,"side":"staff","start":true}),
    );
    no_id_check(&mut s, 1);
    scan(&mut s, 1, "staff", CARTON);
    let v = reg(&s, 1);
    assert!(
        v.staff.lines.iter().all(|l| l.age_checked),
        "20 is already checked"
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    assert!(
        !reg(&s, 1).staff.age_choice_pending,
        "a carton after a lighter (both 20) does not redo the check"
    );
}

#[test]
fn an_18_item_after_a_20_check_redoes_the_check_for_18() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    no_id_check(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"back_to_registering","register":1,"side":"staff"}),
    );
    scan(&mut s, 1, "staff", FIREWORKS);
    let v = reg(&s, 1);
    let fw = v.staff.lines.iter().find(|l| l.age == Some(18)).unwrap();
    assert!(!fw.age_checked);
    assert!(
        v.staff
            .lines
            .iter()
            .find(|l| l.age == Some(20))
            .unwrap()
            .age_checked
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    let v = reg(&s, 1);
    assert!(v.staff.age_choice_pending);
    assert_eq!(v.staff.required_age, Some(18));
    no_id_check(&mut s, 1);
    assert!(reg(&s, 1).staff.lines.iter().all(|l| l.age_checked));
}

#[test]
fn self_operated_emoney_short_calls_staff_only_when_asked() {
    let mut s = Store::new(1, 2, 1);
    short_checkout(&mut s);
    assert_eq!(reg(&s, 1).emoney_short, Some(1_000));
    assert!(reg(&s, 1).staff.calls.is_empty(), "残高不足の時点では呼ばない");
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"customer","choice":"staff"}),
    );
    assert_eq!(reg(&s, 1).emoney_short, Some(1_000));
    assert!(reg(&s, 1).staff.calls.iter().any(|c| c.label == "電子マネー"));
    assert!(
        run(
            &mut s,
            json!({"type":"resolve_emoney_short","register":1,"side":"customer","choice":"cancel"})
        )
        .is_err(),
        "呼んだあとは他の決済に切り替えない（スタッフの無駄足になる）"
    );
    assert!(reg(&s, 1).staff.calls.iter().any(|c| c.label == "電子マネー"));

    // スタッフがログインすると、スタッフ操作モードの残高不足の画面につながり、不足額を現金で払える
    let mut s = Store::new(1, 2, 1);
    short_checkout(&mut s);
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"customer","choice":"staff"}),
    );
    login(&mut s, 1);
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::StaffOperated);
    assert_eq!(v.emoney_short, Some(1_000));
    assert!(v.staff.calls.is_empty());
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"staff","choice":"cash"}),
    );
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"staff","choice":"cash_confirmed"}),
    );
    ok(&mut s, json!({"type":"tap_emoney","register":1,"method":"emoney"}));
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).last_receipt.unwrap().method, "電子マネー・現金");
}

fn pay_cash(s: &mut Store, r: u8) {
    ok(
        s,
        json!({"type":"select_payment","register":r,"side":"staff","method":"cash"}),
    );
    ok(
        s,
        json!({"type":"insert_cash","register":r,"denomination":10000,"count":1}),
    );
    ok(s, json!({"type":"finish_insert","register":r}));
}

#[test]
fn points_are_earned_per_200_yen_of_eligible_goods_and_kept_on_the_card() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", "PC12345678");
    scan(&mut s, 1, "staff", SANDWICH);
    scan(&mut s, 1, "staff", SANDWICH);
    scan(&mut s, 1, "staff", TOBACCO);
    let v = reg(&s, 1);
    assert!(v.points.eligible);
    assert_eq!(v.points.balance, Some(1_250));
    assert_eq!(v.points.earn, 640 / 200, "tobacco earns nothing");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    no_id_check(&mut s, 1);
    pay_cash(&mut s, 1);
    let r = reg(&s, 1).last_receipt.expect("決済済み");
    assert_eq!((r.points, r.points_used, r.point_balance), (3, 0, Some(1_253)));

    scan(&mut s, 1, "staff", BREAD);
    scan(&mut s, 1, "staff", "PC12345678");
    assert_eq!(reg(&s, 1).points.balance, Some(1_253));
}

#[test]
fn a_short_point_balance_pays_part_and_the_rest_goes_to_another_method() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", "PC87654321");
    scan(&mut s, 1, "staff", SANDWICH);
    scan(&mut s, 1, "staff", TOBACCO);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    no_id_check(&mut s, 1);
    assert_eq!(reg(&s, 1).points.usable, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"points"}),
    );
    use_points(&mut s, 1, "all", None);
    let v = reg(&s, 1);
    assert_eq!((v.prepaid, v.staff.due), (1, 919));
    assert_eq!(v.staff.method, None);
    assert_eq!(v.points.earn, (320 - 1) / 200, "no points on the part paid by points");
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"credit"}),
    );
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"credit"}),
    );
    let r = reg(&s, 1).last_receipt.expect("決済済み");
    assert_eq!((r.points, r.points_used, r.point_balance), (1, 1, Some(1)));
}

#[test]
fn voiding_returns_used_points_and_an_empty_card_cannot_pay() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", "PC87654321");
    scan(&mut s, 1, "staff", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"points"}),
    );
    use_points(&mut s, 1, "all", None);
    assert_eq!(reg(&s, 1).points.balance, Some(0));
    ok(
        &mut s,
        json!({"type":"void_transaction","register":1,"side":"staff"}),
    );
    scan(&mut s, 1, "staff", BREAD);
    scan(&mut s, 1, "staff", "PC87654321");
    assert_eq!(reg(&s, 1).points.balance, Some(1));

    // 1P を使い切ると、残高 0 のカードではポイント利用を選べない
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"points"}),
    );
    use_points(&mut s, 1, "all", None);
    pay_cash(&mut s, 1);
    assert_eq!(reg(&s, 1).last_receipt.and_then(|r| r.point_balance), Some(0));
    scan(&mut s, 1, "staff", BREAD);
    scan(&mut s, 1, "staff", "PC87654321");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    assert!(reg(&s, 1)
        .unavailable
        .iter()
        .any(|u| u.method == PaymentMethod::Points));
    assert!(run(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"points"})
    )
    .is_err());
}

#[test]
fn only_non_point_goods_skip_the_point_card_check() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", TOBACCO);
    assert!(!reg(&s, 1).points.eligible);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    no_id_check(&mut s, 1);
    assert!(!reg(&s, 1).staff.point_pending);
}

fn use_points(s: &mut Store, r: u8, how: &str, points: Option<u32>) {
    ok(
        s,
        json!({"type":"use_points","register":r,"side":"staff","how":how,"points":points}),
    );
}

#[test]
fn points_can_pay_all_the_odd_yen_or_a_chosen_amount() {
    let start = |s: &mut Store| {
        login(s, 1);
        scan(s, 1, "staff", "PC12345678");
        scan(s, 1, "staff", SANDWICH);
        scan(s, 1, "staff", BREAD);
        scan(s, 1, "staff", TOBACCO);
        ok(
            s,
            json!({"type":"checkout","register":1,"side":"staff"}),
        );
        no_id_check(s, 1);
        ok(
            s,
            json!({"type":"select_payment","register":1,"side":"staff","method":"points"}),
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
    assert_eq!((v.prepaid, v.staff.due), (80, 1_000));

    let mut s = Store::new(1, 2, 1);
    start(&mut s);
    assert!(run(
        &mut s,
        json!({"type":"use_points","register":1,"side":"staff","how":"amount","points":481})
    )
    .is_err());
    use_points(&mut s, 1, "amount", Some(300));
    let v = reg(&s, 1);
    assert_eq!((v.prepaid, v.staff.due), (300, 780));
    assert_eq!(v.points.balance, Some(950));

    let mut s = Store::new(1, 2, 1);
    start(&mut s);
    use_points(&mut s, 1, "all", None);
    assert_eq!(reg(&s, 1).prepaid, 480);
}
