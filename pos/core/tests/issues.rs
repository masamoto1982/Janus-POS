//! 新型レジで報告された問題と、壁打ちで決めた仕様を、シナリオで検証する。

use janus_pos_core::authority::{Action, Mode, PaymentMethod, Phase, Side, StaffState};
use janus_pos_core::calls::CallKind;
use janus_pos_core::store::{Command, FfState, LineKind, RegisterView, Store};
use serde_json::{json, Value};

const BREAD: &str = "4900000000011";
const BEER: &str = "4900000000103";
/// 店舗ルールの初期値（20 歳）が付いている商品
const LIGHTER: &str = "4900000000134";
/// 年確ルールのない商品。店舗ルールで 18 歳にする例に使う
/// 年確ルールのない商品（オーナーが店舗ルールを付ける例に使う）
const SANDWICH: &str = "4900000000028";
const TEA: &str = "4900000000035";
const PARCEL: &str = "2100000000014";
const PARCEL_COLLECT: &str = "2100000000045";
/// 荷物を受取る
const RECEIVE: &str = "2100000000038";
const BILL: &str = "2100000000021";
const WATER_BILL: &str = "2100000000043";
const GAS_BILL: &str = "2100000000050";
const STAMP: &str = "4900000000141";
const TOBACCO: &str = "4900000000127";

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

/// 名札（スタッフ001）を読んでログインし、店員画面にタッチして操作を引き継ぐ。
/// ログイン中は有人モード（ログイン直後はお客様画面が操作側）
fn login(s: &mut Store, r: u8) {
    login_as(s, r, "ST001");
}

/// お客様側の画面から店舗メニューを開く：店員画面に出た使い捨てパスワードを入力する
fn open_customer_menu(s: &mut Store, r: u8) {
    ok(s, json!({"type":"request_menu_code","register":r}));
    let code = reg(s, r).menu_code.expect("店員画面にパスワードが出る");
    ok(
        s,
        json!({"type":"open_menu","register":r,"side":"customer","screen":"top","code":code}),
    );
}

fn login_as(s: &mut Store, r: u8, badge: &str) {
    scan(s, r, "staff", badge);
    ok(s, json!({"type":"claim","register":r,"side":"staff"}));
}

/// 年確で店員が「しない」を選ぶ（店員の判断で年確が済む。お客様のボタンは挟まない）
fn no_id_check(s: &mut Store, r: u8) {
    ok(
        s,
        json!({"type":"choose_age_check","register":r,"id_check":false}),
    );
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
    scan(&mut s, 1, "staff", BREAD);
    scan(&mut s, 1, "customer", "PC12345678");
    scan(&mut s, 1, "staff", "2000000000015");
    scan(&mut s, 1, "customer", BREAD);
    let v = reg(&s, 1);
    // 間にレジ袋をはさんだパンは、前のパンの行にまとめない
    assert_eq!(v.staff.lines.len(), 3);
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
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":500,"count":1}),
    );
    ok(&mut s, json!({"type":"finish_insert","register":1}));
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
    // 年確が先。ポイントカードの最終確認は年確が済むまで出さない
    assert!(!v.staff.point_pending);
    assert!(run(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":1000,"count":1})
    )
    .is_err());
    // 店員が「しない」を選ぶと、年確はそこで済む（お客様のボタンは挟まない）
    no_id_check(&mut s, 1);
    assert!(!reg(&s, 1).staff.age_choice_pending);
    // 年確が済んでから、ポイントカードの最終確認
    confirm_point(&mut s, 1);
    // 同じ取引で戻って追加しても、再確認はしない
    scan(&mut s, 1, "staff", BEER);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    confirm_point(&mut s, 1);
    assert!(!reg(&s, 1).staff.age_choice_pending);
    ok(
        &mut s,
        json!({"type":"insert_cash","register":1,"denomination":1000,"count":1}),
    );
}

/// セミセルフでは年確が店員呼び出しになり、店員が来るまでお客様は確認できない。
#[test]
fn semi_self_age_check_calls_staff() {
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
    // 係員を待つ間、お客様には身分証の準備を案内する（年齢つき）
    assert_eq!(reg(&s, 2).customer.age_wait, Some(20));
    scan(&mut s, 2, "staff", "ST002");
    // 有人モードになったら案内は消え、店員が年確をするか選ぶ
    assert_eq!(reg(&s, 2).customer.age_wait, None);
    no_id_check(&mut s, 2);
    // 名札を読んだので有人モード。モードはログアウトか 5 分の無操作でだけ戻る
    assert_eq!(reg(&s, 2).mode, Mode::Attended);
    assert_eq!(reg(&s, 2).staff.state, StaffState::Ready);
    assert!(s.snapshot().calls.is_empty());
}

/// 年確の呼び出し中（会計画面）でも、FF ボタンはサービス・たばこと同じく押せる
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

/// 店舗ルール：オーナーだけが追加でき、法令ルールは外せない。
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

/// 問題3・5：有人モードのお客様は流れを変えられず、店員はどのモードでも操作できる。
#[test]
fn customer_cannot_take_over_attended_register() {
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

/// FF：セミセルフの FF ボタンで呼び出し → 名札でログイン（有人）して登録 → お渡し → お客様の受取確認。
#[test]
fn fast_food_flow_in_semi_self_mode() {
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
    // お渡し数が注文数（2個）と合わなければ NG：お渡し前のままで、会計に進めない
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
    // スタッフの照合の後、お客様が「確認しました」を押すまで会計に進めない（相互チェック）
    let e = run(
        &mut s,
        json!({"type":"checkout","register":2,"side":"staff"}),
    )
    .unwrap_err();
    assert!(e.contains("お受け取り"), "{e}");
    ok(
        &mut s,
        json!({"type":"receive_ff","register":2,"side":"customer"}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"staff"}),
    );
    confirm_point(&mut s, 2);
    // 会計画面では照合し直せない
    assert!(run(
        &mut s,
        json!({"type":"check_ff","register":2,"side":"staff","line_id":ff,"handed":2})
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

/// 荷物を受取る：受取用バーコードを読むと係員を呼び、ログインした係員の画面にお預かりの番号が出る。
/// 保管場所の荷物のバーコードを読んで番号が合えば、サインをもらって引き渡し、取引を終える（一件ずつ）
#[test]
fn pickup_matches_the_parcel_then_hands_it_over() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 2, "customer", RECEIVE);
    assert!(s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.kind == CallKind::Service));
    // 二件目の受け取りも、ほかの商品も一緒には扱わない
    for code in [RECEIVE, BREAD, PARCEL] {
        assert!(run(
            &mut s,
            json!({"type":"scan","register":2,"side":"customer","code":code})
        )
        .is_err());
    }
    login_as(&mut s, 2, "ST001");
    assert_eq!(reg(&s, 2).mode, Mode::Attended);
    let no = reg(&s, 2).staff.lines[0]
        .line
        .pickup
        .clone()
        .expect("お預かりの番号")
        .no;
    // 金額は受取用バーコードを読んだときからお客様に見えている
    let amount = reg(&s, 2).staff.lines[0]
        .line
        .pickup
        .clone()
        .unwrap()
        .amount;
    assert!(amount > 0);
    assert_eq!(reg(&s, 2).customer.total, amount);
    // 照合の前は引き渡せず、金額の入力もできない。会計もできない
    assert!(run(&mut s, json!({"type":"pickup_handover","register":2})).is_err());
    let id = reg(&s, 2).staff.lines[0].line.id;
    assert!(run(
        &mut s,
        json!({"type":"complete_service","register":2,"line_id":id,"amount":0})
    )
    .is_err());
    assert!(!allowed(&reg(&s, 2), Side::Staff, Action::Checkout));
    // 別の荷物（番号違い）は照合で止まる。お客様側の画面では読まない
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
    // 照合が済むと代金引換の金額が明細に付き、お客様側にも合計として出る。会計の前には引き渡せない
    let v = reg(&s, 2);
    assert!(pk.amount >= 1_000);
    assert_eq!(v.customer.total, pk.amount);
    assert!(run(&mut s, json!({"type":"pickup_handover","register":2})).is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":2,"side":"staff"}),
    );
    // 代金引換はポイント付与の対象外なので、ポイントカードの確認は出ない
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
    // 決済の後、サインをもらって引き渡す
    assert_eq!(v.pickup_handover.as_deref(), Some(pk.no.as_str()));
    ok(&mut s, json!({"type":"pickup_handover","register":2}));
    let v = reg(&s, 2);
    assert!(v.staff.lines.is_empty() && v.pickup_handover.is_none());
    assert!(!s
        .snapshot()
        .calls
        .iter()
        .any(|c| c.kind == CallKind::Service && c.register == 2));
    // 引き渡しの後は、続けてお買い物ができる
    scan(&mut s, 2, "staff", BREAD);
}

/// シミュレーターでは、荷物のバーコードを読む代わりに「有り」「無し」の二択で進められる。
/// 「無し」なら引き渡さずにお受け取りを終え、「探し直す」で照合の前に戻る。
/// 「有り」の後は探し直さず、明細も取り消せない（受け取らないなら取引中止）
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
    // 無しならお受け取りを終える（別の取引で、もう一度読んでから有りにする）
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

/// 荷物を送る：着払いはレジで 0 円。登録済みの宅配便は、伝票のバーコードを読んで取り消す
/// （明細の取り消しボタンでは取り消せない。お客様側では読まない）
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
    // 次の荷物は同じ着払いで受け付ける
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
    // 明細の取り消しボタンでは取り消せない
    assert!(run(
        &mut s,
        json!({"type":"remove_line","register":2,"side":"staff","line_id":id})
    )
    .is_err());
    // お客様側では読まない。違う伝票でも取り消さない
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

/// 荷物を送る：サイズは二段階（三辺の合計 → 重さ）で決める。「戻る」は一段階ずつ引き返し、
/// 三辺の合計から戻るとその荷物の受付をやめる（受付済みの荷物があれば「次の荷物はありますか」に戻る）
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
    // 郵便番号 → 重さ → 三辺の合計 と一段階ずつ戻る
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
    // 三辺の合計から戻ると、この荷物の受付をやめて登録画面に戻る
    ok(
        &mut s,
        json!({"type":"parcel_back","register":2,"line_id":id}),
    );
    assert!(reg(&s, 2).staff.lines.is_empty());
    assert!(!reg(&s, 2).staff.parcel_ask);
    // 二個目で戻ると「次の荷物はありますか」に戻る
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

/// 荷物を送る：伝票を読むと係員を呼び、係員がサイズ（三辺の合計・重さ）→ 郵便番号の順に決めると料金が決まる。
/// 次の荷物が「ある」ならもう一度サイズから、「ない」なら会計に進める（支払方法の制限なし）
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
    // 金額の直接入力はできない
    assert!(run(
        &mut s,
        json!({"type":"complete_service","register":2,"line_id":id,"amount":1200})
    )
    .is_err());
    // サイズを決める前に郵便番号は決められない
    assert!(run(
        &mut s,
        json!({"type":"parcel_postal","register":2,"line_id":id,"postal":"100-0001"})
    )
    .is_err());
    // 三辺の合計 200cm・重さ 30kg を超えると受け付けない（上限は数字で案内する）
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
    // 次の荷物がある：もう一つ受付する
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
    // 80 サイズ・関西あて
    assert_eq!(reg(&s, 2).customer.total, 940 + 1230 + 240);
    ok(
        &mut s,
        json!({"type":"parcel_next","register":2,"more":false}),
    );
    let v = reg(&s, 2);
    assert!(!v.staff.parcel_ask);
    assert!(allowed(&v, Side::Staff, Action::Checkout));
    // ポイントカードを読んでも、宅配便はポイント付与の対象外（会計時のポイントカードの確認も出ない）
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

/// 宅配便だけの会計では、ポイントカードの確認を出さない
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

/// 荷物の発送は、ほかの商品と一緒に会計できない（荷物どうしは一緒に会計できる）
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
    // 係員が受付をするまで会計できない
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

/// 問題6：出金はお客様側の店舗メニューに入ったときだけでき（入るには使い捨てパスワード）、記録が残る。
#[test]
fn cash_out_only_inside_the_customer_side_menu() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    let co = json!({"type":"cash_out","register":1,"kind":"emergency","amount":5000});
    assert!(
        run(&mut s, co.clone()).is_err(),
        "店舗メニューの外では出金できない"
    );
    // 店員側の店舗メニューにはお金の出し入れがない
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":"top"}),
    );
    assert!(run(&mut s, co.clone()).is_err());
    assert!(run(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":"emergency"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":null}),
    );
    open_customer_menu(&mut s, 1);
    ok(&mut s, co.clone());
    ok(&mut s, co);
    let a = s.snapshot().audit;
    assert_eq!(a.len(), 2);
    assert_eq!(a[1].staff, "スタッフ001");
    // 未ログインのレジでは出金できない
    assert!(run(
        &mut s,
        json!({"type":"cash_out","register":2,"kind":"refund","amount":100})
    )
    .is_err());
}

/// お客様側から店舗メニューを開くには、店員画面にだけ出る使い捨てのパスワード（英大文字と数字の 4 桁）が要る。
/// 店員側からはパスワードなしで開ける。開いた側の画面にだけ、その側の項目を出す。
#[test]
fn customer_side_menu_needs_a_one_time_code() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    let open = |code: &str| json!({"type":"open_menu","register":1,"side":"customer","screen":"top","code":code});
    assert!(
        run(&mut s, open("AAAA")).is_err(),
        "パスワードを出す前は開けない"
    );
    ok(&mut s, json!({"type":"request_menu_code","register":1}));
    let v = reg(&s, 1);
    let code = v.menu_code.clone().unwrap();
    assert_eq!(code.len(), 4);
    assert!(code
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    assert_eq!(v.active_side, Some(Side::Customer), "お客様側で入力する");
    let wrong = if code == "0000" { "1111" } else { "0000" };
    assert!(run(&mut s, open(wrong)).is_err());
    ok(&mut s, open(&code.to_lowercase()));
    let v = reg(&s, 1);
    assert_eq!(v.menu.unwrap().side, Side::Customer);
    assert!(v.menu_code.is_none(), "パスワードは一度きり");
    assert_eq!(v.active_side, Some(Side::Customer));
    // お客様側には店員側の項目を出さない
    assert!(run(
        &mut s,
        json!({"type":"open_menu","register":1,"side":"customer","screen":"journal"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"side":"customer","screen":null}),
    );
    // 閉じたら、もう一度パスワードを出さないと開けない
    assert!(run(&mut s, open(&code)).is_err());
    // 続けて間違えるとロックし、パスワードを作り直す
    ok(&mut s, json!({"type":"request_menu_code","register":1}));
    let first = reg(&s, 1).menu_code.unwrap();
    let bad = if first == "ZZZZ" { "YYYY" } else { "ZZZZ" };
    for _ in 0..3 {
        assert!(run(&mut s, open(bad)).is_err());
    }
    let again = reg(&s, 1).menu_code.unwrap();
    assert!(run(&mut s, open(&again)).is_err(), "しばらくロック");
    advance(&mut s, 31_000);
    ok(&mut s, open(&again));
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"side":"customer","screen":null}),
    );
    advance(&mut s, 1_000);
    // 店員側はパスワードなし。店員側の項目（仮設）を開ける
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":"top"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.menu.unwrap().side, Side::Staff);
    assert_eq!(v.active_side, Some(Side::Staff));
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":"journal"}),
    );
    // 有人モードのお客様側のバッジはいつでも押せる：店員側のメニューは閉じ、パスワードを出す
    ok(&mut s, json!({"type":"request_menu_code","register":1}));
    let v = reg(&s, 1);
    assert!(v.menu.is_none());
    assert!(v.menu_code.is_some());
    assert_eq!(v.active_side, Some(Side::Customer));
    // 店員側のバッジもいつでも押せる：お客様側のパスワード入力は取りやめになる
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":"top"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.menu.unwrap().side, Side::Staff);
    assert!(v.menu_code.is_none());
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
    login_as(&mut s, 4, "ST002");
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
    assert_eq!(
        snap.registers[0].staff.mat_down,
        vec!["据え付け端末".to_string()]
    );
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
    scan(&mut s, 1, "customer", BREAD);
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SemiSelf);
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
    // 店員が名札を読むと有人モードになり、支払い中でも両方の画面が操作できる
    scan(&mut s, 1, "staff", "ST002");
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
    assert!(run(&mut s, set("001", "1111", SANDWICH, Some(20))).is_err());
    ok(&mut s, set("000", "9999", SANDWICH, Some(20)));
    ok(&mut s, set("000", "9999", LIGHTER, None));
    // 酒・たばこは法令どおりで変更不可
    assert!(run(&mut s, set("000", "9999", BEER, None)).is_err());
    assert!(run(&mut s, set("000", "9999", BEER, Some(25))).is_err());
    // どの商品にも個別に設定できる
    ok(&mut s, set("000", "9999", TEA, Some(18)));
    let cat = s.snapshot().catalog;
    let age = |c: &str| cat.iter().find(|p| p.code == c).unwrap().age;
    assert_eq!(age(SANDWICH), Some(20));
    assert_eq!(age(LIGHTER), None);
    assert_eq!(age(BEER), Some(20));
    assert_eq!(age(TEA), Some(18));
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
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SemiSelf);
}

/// セミセルフでは、お客様は FF を登録できない（FF ボタンで係員を呼ぶ）。
#[test]
fn customer_cannot_register_ff_in_semi_self() {
    let mut s = Store::new(1, 2, 1);
    assert!(run(
        &mut s,
        json!({"type":"register_ff","register":2,"side":"customer","code":"FF01","qty":1})
    )
    .is_err());
    ok(&mut s, json!({"type":"ff_button","register":2}));
}

/// 名札でログインした有人モードも、5 分間操作がなければログアウトしてセミセルフに戻る。
/// 商品を登録していても、5 分間操作がなければ取引を中止して戻る（取引の保留機能は持たない）。
#[test]
fn idle_register_returns_to_semi_self() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 2);
    advance(&mut s, 4 * 60_000);
    // 途中で操作があれば数え直す
    scan(&mut s, 2, "staff", BREAD);
    ok(&mut s, json!({"type":"void_transaction","register":2}));
    advance(&mut s, 4 * 60_000);
    assert_eq!(reg(&s, 2).mode, Mode::Attended);
    advance(&mut s, 61_000);
    let v = reg(&s, 2);
    assert_eq!(v.mode, Mode::SemiSelf);
    assert_eq!(v.staff.state, StaffState::Absent);
    assert!(v.idle_revert_in.is_none());
    // 取引中でも 5 分間操作がなければ、取引を中止してセミセルフに戻る。入れたお金は出金ボックスへ
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
        Mode::Attended,
        "財布を取りに行くくらいは待つ"
    );
    advance(&mut s, 61_000);
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SemiSelf);
    assert!(v.customer.lines.is_empty());
    assert_eq!(v.customer.inserted, 0);
    assert_eq!(v.outlet, 100);
    // セミセルフで登録したまま離れた取引も、5 分で中止する
    scan(&mut s, 2, "customer", BREAD);
    assert!(reg(&s, 2).idle_voids);
    advance(&mut s, 5 * 60_000 + 1_000);
    assert!(reg(&s, 2).customer.lines.is_empty());
    assert!(reg(&s, 2).idle_revert_in.is_none());
}

/// 店舗メニュー（お客様側）：お金を入れる操作も抜く操作も、メニューに入ればパスワード不要。どちらも記録が残る。
#[test]
fn cash_in_and_out_from_the_customer_side_menu() {
    let mut s = Store::new(1, 2, 1);
    let cin = json!({"type":"cash_in","register":1,"kind":"deposit","amount":3000});
    // 名札でログインした有人モードでだけ使える
    assert!(run(&mut s, cin.clone()).is_err());
    login(&mut s, 1);
    assert!(
        run(&mut s, cin.clone()).is_err(),
        "店舗メニューの外では入金できない"
    );
    open_customer_menu(&mut s, 1);
    let before = reg(&s, 1).staff.drawer_total;
    ok(&mut s, cin);
    ok(
        &mut s,
        json!({"type":"cash_in","register":1,"kind":"changer_refill","amount":1500}),
    );
    assert_eq!(reg(&s, 1).staff.drawer_total, before + 4500);
    ok(
        &mut s,
        json!({"type":"cash_out","register":1,"kind":"changer_collect","amount":4500}),
    );
    assert_eq!(reg(&s, 1).staff.drawer_total, before);
    let ops: Vec<_> = s.snapshot().audit.iter().map(|a| a.operation).collect();
    assert_eq!(ops, vec!["取引外入金", "釣銭補充", "釣銭機回収"]);
    // 取引中は入出金できない
    scan(&mut s, 1, "staff", BREAD);
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
    scan(&mut s, 1, "customer", BREAD);
    // お会計前は受け付けない
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":1000})).is_err());
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    // 入れたお金は積み上がり、Enter で投入を終えると決済する
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
    // 「ありがとうございました」は 3 秒で消え、新しいお客様の待ち受けに戻る。
    // ただし排出口に釣り銭が残っている間は、取り出されるまで戻らない
    assert!(v.last_receipt.is_some());
    let at = v.last_receipt.as_ref().unwrap().at;
    s.tick(at + 3000);
    assert!(reg(&s, 1).last_receipt.is_some());
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    assert_eq!(reg(&s, 1).outlet, 0);
    s.tick(at + 3001);
    assert!(reg(&s, 1).last_receipt.is_none());
    assert!(run(&mut s, json!({"type":"take_outlet","register":1})).is_err());
}

/// お客様側の店舗メニューは一画面ずつ出す。開いている間はお客様画面側で操作し、
/// 入金・両替は投入口、出金と両替のお金は排出口を使う。
#[test]
fn store_menu_uses_inlet_and_outlet() {
    let mut s = Store::new(1, 2, 1);
    let open =
        |screen: &str| json!({"type":"open_menu","register":1,"side":"customer","screen":screen});
    assert!(
        run(&mut s, json!({"type":"request_menu_code","register":1})).is_err(),
        "only for logged-in staff"
    );
    login(&mut s, 1);
    open_customer_menu(&mut s, 1);
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
        json!({"type":"open_menu","register":1,"side":"customer","screen":"changer","exchange_to":1000}),
    );
    assert!(run(&mut s, json!({"type":"insert","register":1,"amount":1500})).is_err());
    ok(&mut s, json!({"type":"insert","register":1,"amount":10000}));
    let v = reg(&s, 1);
    assert_eq!(v.outlet_detail, vec![(1000, 10)]);
    assert_eq!(v.staff.drawer_total, before + 2000);
    ok(&mut s, json!({"type":"take_outlet","register":1}));
    // 緊急出金：お金は排出口に出る
    ok(&mut s, open("emergency"));
    ok(
        &mut s,
        json!({"type":"cash_out","register":1,"kind":"emergency","amount":5000}),
    );
    assert_eq!(reg(&s, 1).outlet, 5000);
    // 閉じると両方の画面に戻る（触れている一瞬が過ぎれば）。ログアウトでもメニューは閉じる
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"side":"customer","screen":null}),
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

/// 有人モード：どちらも触れていなければ、FF はどちらの画面からも登録できる。
/// 触れている一瞬だけ、もう一方の画面からの操作が利かない。
#[test]
fn attended_screens_lock_only_while_touched() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "staff", "ST001");
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
    let check = |id: u32, side: &str| json!({"type":"check_ff","register":1,"side":side,"line_id":id,"handed":1});
    // 店員が触れた直後は、お客様画面からはチェックできない
    assert!(run(&mut s, check(ids[0], "customer")).is_err());
    ok(&mut s, check(ids[0], "staff"));
    assert!(!allowed(&reg(&s, 1), Side::Staff, Action::Checkout));
    advance(&mut s, 1_000);
    ok(&mut s, check(ids[1], "customer"));
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"receive_ff","register":1,"side":"customer"}),
    );
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

/// 財布のクレジットカードで払う。選んだ支払方法と違うカードをかざしても通らない。
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
    // 宅配便のバーコードを直接読むと、サービスの呼び出しになる
    scan(&mut s, 2, "customer", PARCEL);
    assert!(kinds(&s, 2).contains(&CallKind::Service));
    // 店員が来ると、たばこ・サービスのボタン呼び出しは片付く（FF は登録するまで残る）
    login(&mut s, 1);
    assert_eq!(kinds(&s, 1), vec![CallKind::FastFood]);
    // 有人モードでは、店員が仲間に SOS を出すのに使う。Bell は鳴るが、呼び出しの記録は増えない
    ok(&mut s, json!({"type":"call_staff","register":1}));
    advance(&mut s, 1_000);
    assert!(s.snapshot().mats[0].buzzing, "SOS rings the bell");
    assert_eq!(kinds(&s, 1), vec![CallKind::FastFood]);
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
    scan(&mut s, 1, "staff", "ST001");
    assert_eq!(reg(&s, 1).mode, Mode::Attended);
    assert_eq!(reg(&s, 1).active_side, None);
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SemiSelf);
    // 取引の途中（FF 登録済み）でも戻れる
    scan(&mut s, 1, "staff", "ST001");
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
    scan(&mut s, 1, "staff", "ST001");
    assert_eq!(reg(&s, 1).active_side, None);
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"customer","code":"FF01","qty":1}),
    );
    let id = reg(&s, 1).customer.lines[0].id;
    ok(
        &mut s,
        json!({"type":"check_ff","register":1,"side":"customer","line_id":id,"handed":1}),
    );
    ok(
        &mut s,
        json!({"type":"receive_ff","register":1,"side":"customer"}),
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
    ok(&mut s, json!({"type":"finish_insert","register":1}));
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
        json!({"type":"check_ff","register":1,"side":"customer","line_id":id,"handed":1}),
    );
    ok(
        &mut s,
        json!({"type":"receive_ff","register":1,"side":"customer"}),
    );
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    assert!(!reg(&s, 1).customer.point_pending);
}

/// 店員側・お客様側の区別はない。有人モードでは操作側の画面なら取引中止もできる。
/// 年確の「する／しない」は店員側の画面にだけ効く。
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
    // 取引中止もお客様側の画面から（操作側なので）できる
    ok(&mut s, json!({"type":"void_transaction","register":1}));
}

/// 有人モードで店員が年確を「しない」を選ぶと、年確はそこで済む。
/// お客様側の画面に年確のボタンは出さず、そのままポイントカードの最終確認へ進む。
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

/// 店員が払込票の種類ごとの金額を確かめ、チェックを入れる（false で外す）
fn check_bill(s: &mut Store, r: u8, line_id: u32, checked: bool) {
    ok(
        s,
        json!({"type":"check_bill","register":r,"side":"staff","line_id":line_id,"checked":checked}),
    );
}

/// 払込票の明細番号（読み取った順）
fn bill_ids(s: &Store, r: u8) -> Vec<u32> {
    reg(s, r).staff.bills.iter().map(|b| b.line_id).collect()
}

/// 公共料金の支払い：払込票は普通の商品と同じに読み取り、会計ボタンの後に店員が払込票の内容を
/// 一枚ずつ事業者・金額を確かめてチェックを入れる（店員側の画面だけ。同じ事業者でもまとめない）。
/// すべて確認するまで支払いに進めない。
/// 現金のみで、ポイントの確認・年確はない。決済の後は、店員が収納印を押して「お渡し済」を押すまで
/// お客様に払込票・レシート・お釣りの受け取りを案内する
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
            ("電気料金", 5_000),
            ("水道料金", 4_000),
            ("ガス料金", 3_000),
            ("電気料金", 5_000)
        ]
    );
    // 払込票ごとに便宜的なバーコード番号（16 桁を 4 桁ずつ）。同じ事業者でも別の番号
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
    // チェックは外せる（外せば未確認に戻る）
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
    ok(&mut s, json!({"type":"insert","register":1,"amount":20000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    let v = reg(&s, 1);
    let r = v.last_receipt.unwrap();
    assert_eq!((r.total, r.method, r.points), (17_000, "現金", 0));
    assert_eq!(v.bill_handover, Some(4));
    // お釣りを取り出しても、お渡し済を押すまで案内は残る
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

/// セミセルフでは、会計ボタンで係員を呼ぶ（呼び出しは「公共料金」と枚数）。係員がログインして
/// 払込票の金額を確認するまで、お客様は支払いに進めない
#[test]
fn semi_self_bill_checkout_calls_staff_to_check_the_slips() {
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
    assert_eq!(reg(&s, 1).mode, Mode::Attended);
    let ids = bill_ids(&s, 1);
    check_bill(&mut s, 1, ids[0], true);
    assert!(!s.snapshot().calls.is_empty());
    check_bill(&mut s, 1, ids[1], true);
    assert!(s.snapshot().calls.is_empty());
    ok(&mut s, json!({"type":"insert","register":1,"amount":10000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    assert_eq!(reg(&s, 1).bill_handover, Some(2));
    // 次のお買い物が始まれば、お渡しは済んだものとする
    scan(&mut s, 1, "staff", BREAD);
    assert_eq!(reg(&s, 1).bill_handover, None);
}

/// 公共料金の払込票は、ほかの商品と一緒に登録できない（どちらを先に読んでも、FF も）
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

/// 登録した払込票は明細ごとには取り消せない（有人でもセミセルフでも）。
/// 払えないときは取引中止で最初からやり直す
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

/// ポイントはポイント付与の対象商品の分だけに使える（付く商品＝使える商品）。
/// FF・サンドイッチ・たばこなら、ポイントで払うのはたばこ以外。残りのたばこはほかの支払方法で払う
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
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"points"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.prepaid, sand);
    assert_eq!(v.staff.due, tob);
    // 残りのたばこは、ポイント以外の手段で払う
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
    // ポイントで払った分にもポイントは付く対象だが、付与は対象商品（サンドイッチ）の分だけ
    assert_eq!(r.points, sand / 100);

    // たばこだけならポイントは使えない
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

/// 切手は現金のみ（非課税）。ほかの商品と一緒に買えるが、キャッシュレスで払えるのは切手を除いた分だけ
#[test]
fn stamps_are_paid_in_cash_even_with_other_items() {
    // 切手だけならキャッシュレスは選べない
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    // 切手はポイント付与の対象外なので、切手だけの会計ではポイントカードの確認を出さない
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

    // パンと切手：電子マネーで払うのはパンの分だけ、切手は現金で
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    scan(&mut s, 1, "customer", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"emoney"}),
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

    // 取引中止なら電子マネーで払った分は残高に戻る
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    scan(&mut s, 1, "customer", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"customer","method":"emoney"}),
    );
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"}),
    );
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    assert_eq!(emoney_balance(&s), 1_000);
}

/// 買い物点数は 10 点まで（数量の合計）。上限を超える商品は登録されず、FF も同じ。
#[test]
fn shopping_is_limited_to_ten_items() {
    let mut s = Store::new(1, 2, 1);
    for _ in 0..9 {
        scan(&mut s, 1, "customer", BREAD);
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
        json!({"type":"scan","register":1,"side":"customer","code":BREAD}),
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
    scan(&mut s, 1, "customer", BREAD);
}

/// 取引中止・ログアウトは店舗メニューから行い、済んだらメニューは閉じる。ロックはない。
#[test]
fn void_and_logout_from_the_store_menu() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "customer", BREAD);
    let top = json!({"type":"open_menu","register":1,"screen":"top"});
    ok(&mut s, top.clone());
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    let v = reg(&s, 1);
    assert!(v.menu.is_none() && v.staff.lines.is_empty());
    ok(&mut s, top);
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    assert_eq!(reg(&s, 1).mode, Mode::SemiSelf);
}

/// 年確：年確商品があれば、まず店員が「年齢確認をする／しない」を選ぶ。
/// 「する」なら身分証の生年月日を入力し、年齢に届かない年確商品は買い物から取り除いて
/// 回収する商品と個数を店員に示す（店舗判断で 18 歳以上にしたライターは買えるが酒は買えない等）。
#[test]
fn staff_enters_the_birth_date_and_unsellable_items_are_confiscated() {
    let mut s = Store::new(1, 2, 1);
    // 店舗判断でライターを 18 歳以上にしておく
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
    // 選ぶ前は入力できない。お客様側の画面からは選べない
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
    // 19 歳：ライター（18）は買えるが、アルコール（20）は買えない
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
    // 西暦 8 桁だけ。存在しない日付や和暦は読まない
    for bad in ["2006/10/19", "20070231", "570102", "S57.10.2", "2006101"] {
        assert!(age_on(bad, "2026-10-19").is_err(), "{bad}");
    }
}

/// 引換券・割引券は商品より先に読んでもよい。対象商品が登録されたときに値引きし、
/// 対象商品がなければ「対象商品待ち」のまま決済の邪魔はしない。
#[test]
fn coupons_apply_whenever_the_target_is_registered() {
    let mut s = Store::new(1, 2, 1);
    // 商品より先にクーポン
    scan(&mut s, 1, "customer", "CP0001");
    let v = reg(&s, 1);
    assert_eq!(v.customer.coupons.len(), 1);
    assert_eq!(v.customer.coupons[0].discount, None);
    assert_eq!(v.customer.total, 0);
    scan(&mut s, 1, "customer", TEA);
    let v = reg(&s, 1);
    assert_eq!(v.customer.coupons[0].discount, Some(150));
    assert_eq!(v.customer.total, 0);
    // 同じクーポンを 2 枚目も使える。対象商品 1 個につき 1 枚
    scan(&mut s, 1, "customer", "CP0001");
    assert_eq!(reg(&s, 1).customer.coupons[1].discount, None);
    scan(&mut s, 1, "customer", TEA);
    let v = reg(&s, 1);
    assert_eq!(v.customer.coupons[1].discount, Some(150));
    assert_eq!(v.customer.total, 0);
    // 商品の後にクーポン（割引券）。3 本目のソフトドリンクは定価
    scan(&mut s, 1, "customer", SANDWICH);
    scan(&mut s, 1, "customer", TEA);
    scan(&mut s, 1, "customer", "CP0002");
    let v = reg(&s, 1);
    assert_eq!(v.customer.total, 150 + 320 - 50);
    // クーポンは買い物点数に数えない
    assert_eq!(v.staff.lines.iter().map(|l| l.line.qty).sum::<u32>(), 4);
    // クーポンを使ったまま会計できる
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
    // ソフトドリンクの引換券のつもりで、別の商品をレジに通した
    scan(&mut s, 1, "customer", "CP0001");
    scan(&mut s, 1, "customer", "4900000000028");
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"customer"}),
    );
    confirm_point(&mut s, 1);
    // 両方の画面に注意を出し、決済できない
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
    // お会計中でもクーポンを取り消せる。取り消したクーポンは返却の案内に載る
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

/// 同じ商品は、続けて読んだときだけ一番上の行に足す。間に別の商品をはさんだら新しい行にし、
/// 前に登録した行を一番上に積みなおさない（個数を変える人が前の分を覚えていなくて済む）。
#[test]
fn same_item_merges_only_when_scanned_in_a_row() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    scan(&mut s, 1, "customer", BREAD);
    scan(&mut s, 1, "customer", SANDWICH);
    scan(&mut s, 1, "customer", BREAD);
    scan(&mut s, 1, "customer", BREAD);
    scan(&mut s, 1, "customer", BREAD);
    let v = reg(&s, 1);
    let mut lines: Vec<_> = v.customer.lines.iter().collect();
    lines.sort_by_key(|l| std::cmp::Reverse(l.touched));
    let rows: Vec<_> = lines.iter().map(|l| (l.name, l.qty)).collect();
    assert_eq!(rows, [("パン", 3), ("サンドイッチ", 1), ("パン", 2)]);
}

/// 明細の個数を手で変えられる。変えた行は一番上に出て、点数の上限も守る。
#[test]
fn staff_or_customer_changes_a_line_quantity() {
    let mut s = Store::new(1, 2, 1);
    scan(&mut s, 1, "customer", BREAD);
    scan(&mut s, 1, "customer", SANDWICH);
    let id = reg(&s, 1).customer.lines[0].id;
    let set =
        |qty: u32| json!({"type":"set_qty","register":1,"side":"customer","line_id":id,"qty":qty});
    ok(&mut s, set(3));
    let v = reg(&s, 1);
    let top = v.customer.lines.iter().max_by_key(|l| l.touched).unwrap();
    assert_eq!((top.name, top.qty), ("パン", 3));
    assert_eq!(v.customer.total, 160 * 3 + 320);
    assert!(run(&mut s, set(10)).is_err(), "over the 10-item limit");
    assert!(run(&mut s, set(0)).is_err(), "cancel is a separate button");
    ok(&mut s, set(1));
    assert_eq!(reg(&s, 1).customer.total, 160 + 320);
}

/// クーポンと明細の番号は重ならない（取消・変更の対象を取り違えない）。
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

/// 決済手段に「ポイント利用」がある。店員が選び、ポイントカード・アプリを入金ボックスにかざすと決済になる。
#[test]
fn points_can_pay_once_staff_chooses_them() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    // ポイント利用にはポイントカードが要る（読まずに選ぶとポイントカードの確認に差し戻す）
    scan(&mut s, 1, "staff", "PC12345678");
    // 選ぶ前にかざしても決済にならない
    assert!(run(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"points"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"select_payment","register":1,"side":"staff","method":"points"}),
    );
    // 選んだ手段と違うものをかざしても決済にならない
    assert!(run(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"emoney"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"tap_emoney","register":1,"method":"points"}),
    );
    assert!(reg(&s, 1).staff.lines.is_empty());
}

/// 決済手段でポイント利用を選んでも、ポイントカードが読まれていなければポイントカードの確認に差し戻す。
/// 読めばポイント利用のまま支払いへ。読まずに「スキップ」ならポイント利用は取り消す。
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
    // 読めばポイント利用のまま支払いへ
    scan(&mut s, 1, "staff", "PC12345678");
    let v = reg(&s, 1);
    assert!(!v.staff.point_pending);
    assert_eq!(v.staff.phase, Phase::Paying);
    assert_eq!(v.staff.method, Some(PaymentMethod::Points));

    // 別の会計：差し戻しでスキップすれば、ポイント利用は取り消す
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

/// Bell は据え付けの呼び鈴・検品端末・ストアコンピューター。検品端末とストアコンピューターは
/// 設定で呼び鈴機能を OFF にでき、OFF の端末は鳴らず不通の警告も出さない。据え付けは OFF にできない。
#[test]
fn handheld_and_store_computer_bells_can_be_turned_off() {
    let mut s = Store::new(1, 2, 3);
    let ids: Vec<String> = s.snapshot().mats.iter().map(|m| m.id.clone()).collect();
    assert_eq!(ids, ["据え付け端末", "可搬性端末", "ストアコンピューター"]);
    assert!(run(&mut s, json!({"type":"set_bell_on","mat":0,"on":false})).is_err());
    ok(&mut s, json!({"type":"set_bell_on","mat":1,"on":false}));
    ok(&mut s, json!({"type":"set_bell_on","mat":2,"on":false}));
    ok(&mut s, json!({"type":"call_staff","register":1}));
    advance(&mut s, 1_000);
    let snap = s.snapshot();
    assert!(snap.mats[0].buzzing);
    assert!(!snap.mats[1].buzzing && !snap.mats[2].buzzing);
    assert!(!snap.mats[1].bell_on);
    // OFF の端末は不通として警告しない
    advance(&mut s, 8_000);
    assert!(reg(&s, 1).staff.mat_down.is_empty());
    // ON に戻すと、押されている間は鳴る
    ok(&mut s, json!({"type":"set_bell_on","mat":2,"on":true}));
    advance(&mut s, 1_000);
    assert!(s.snapshot().mats[2].buzzing);
}

/// たばこボタンで呼ばれた店員がログインすると、たばこを登録するまで tobacco_asked が立つ（Item のたばこを点滅させる）。
#[test]
fn tobacco_call_marks_the_register_until_tobacco_is_scanned() {
    let mut s = Store::new(1, 2, 1);
    ok(
        &mut s,
        json!({"type":"call_staff","register":1,"topic":"tobacco"}),
    );
    assert!(!reg(&s, 1).tobacco_asked, "semi-self: not yet");
    login(&mut s, 1);
    assert!(reg(&s, 1).tobacco_asked);
    scan(&mut s, 1, "staff", BREAD);
    assert!(reg(&s, 1).tobacco_asked);
    scan(&mut s, 1, "staff", "4900000000127");
    assert!(!reg(&s, 1).tobacco_asked);
    // ボタンを使わずにログインしただけなら立たない
    login(&mut s, 2);
    assert!(!reg(&s, 2).tobacco_asked);
}

/// 釣り銭を取り忘れたまま一定時間でお客様画面に案内、さらにたつと店員呼び出し（釣銭忘れ）。
/// 有人モードは 30 秒・1 分、セミセルフは 1 分・2 分。
/// 残っている間は無操作でセミセルフに戻るカウントダウンを止め、取り出すと続きから再開する。
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
    assert!(v.change_left, "attended, 30 seconds: remind the customer");
    assert!(v.staff.calls.is_empty());
    assert_eq!(v.idle_revert_in, Some(frozen), "countdown is paused");
    advance(&mut s, 30_000);
    let v = reg(&s, 1);
    assert_eq!(v.staff.calls.len(), 1, "attended, 1 minute: call staff");
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
        reg(&s, 1).last_receipt.is_none(),
        "back to the initial screen"
    );
    advance(&mut s, 10_000);
    assert!(
        reg(&s, 1).idle_revert_in.unwrap() < frozen,
        "countdown resumes"
    );

    // セミセルフ：1 分で案内、2 分で呼び出し
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
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    advance(&mut s, 40_000);
    assert!(!reg(&s, 1).change_left);
    advance(&mut s, 21_000);
    assert!(reg(&s, 1).change_left);
    assert!(reg(&s, 1).staff.calls.is_empty());
    advance(&mut s, 60_000);
    assert_eq!(reg(&s, 1).staff.calls[0].label, "釣銭忘れ");
    assert!(reg(&s, 1).last_receipt.is_some());
}

/// 販売個数の上限：オーナーが商品ごとに「1 回のお会計で何点まで」を決める。
/// 上限を超える登録（スキャン・個数の変更）は受け付けない。
#[test]
fn owner_limits_how_many_of_an_item_one_purchase_can_have() {
    let mut s = Store::new(1, 2, 1);
    let limit = |pw: &str, n: Option<u32>| json!({"type":"set_sale_limit","owner_id":"000","password":pw,"code":TEA,"limit":n});
    // オーナーのパスワードでなければ変えられない
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
    // ほかの商品は買える。個数の変更でも上限を超えられない
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
    // 上限を外せば、また買える
    ok(&mut s, limit("9999", None));
    scan(&mut s, 1, "customer", TEA);
    assert!(s
        .snapshot()
        .age_rules
        .history
        .iter()
        .any(|h| h.change.contains("2点まで → 制限なし")));
}

/// 税率設定：税率は区分のデータで持ち、法改正で税率が変わってもオーナーが区分を変えるだけで済む。
/// 登録済みの明細は登録した時点の税率のまま。税率ごとに 1 回だけ端数処理する
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
    // 店員は変えられない
    assert!(run(
        &mut s,
        json!({"type":"set_tax_rate","owner_id":"001","password":"1111","class_id":"standard","rate_bp":1200})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"set_tax_rate","owner_id":"000","password":"9999","class_id":"standard","rate_bp":1200}),
    );
    // 登録済みのアルコールは 10% のまま、あとから登録した分は 12%
    scan(&mut s, 1, "staff", TEA);
    scan(&mut s, 1, "staff", BEER);
    let t = reg(&s, 1).staff.taxes;
    let rates: Vec<&str> = t.iter().map(|x| x.rate.as_str()).collect();
    assert_eq!(rates, vec!["8%", "10%", "12%"]);
    assert_eq!(t[2].tax, 248 * 1200 / 11200);
    // ゼロ税率は 0% から変えられない。インボイスの端数処理を四捨五入に
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

/// 商品ごとに税区分を付け替えられる。代行収納（お支払い）は初めから消費税の対象外
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

/// 「電子マネー」ボタンでチャージか残高確認を選ぶ。残高確認はかざすと残高が出る。
/// セミセルフではお買い物の途中にチャージ・残高確認はできない
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

    // 残高確認だけの取引
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

/// チャージ：紙幣だけを入れてかざす。上限 20,000 円。やめると紙幣は排出口に戻る
#[test]
fn emoney_charge_takes_bills_up_to_the_limit() {
    let mut s = Store::new(1, 2, 1);
    ok(
        &mut s,
        json!({"type":"start_emoney","register":1,"side":"customer","kind":"charge"}),
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
        json!({"type":"start_emoney","register":1,"side":"customer","kind":"charge"}),
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":2000}));
    ok(
        &mut s,
        json!({"type":"close_emoney","register":1,"side":"customer"}),
    );
    assert_eq!(reg(&s, 1).outlet, 2000);
    assert_eq!(emoney_balance(&s), 16_000);
}

/// 残高不足の会計に進めるまで（7 個 × 160 円 = 1,120 円、残高 1,000 円）
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

/// 残高不足は「使うのをやめる（他の決済）」「残りを現金で払う」に分かれる。
/// 「チャージする」は有人モードだけ（セミセルフはお買い物の途中にチャージしない）
#[test]
fn emoney_short_offers_three_choices() {
    // やめる：支払方法の選び直しへ
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

    // 不足分を現金で：案内を確認してから電子マネーをかざして残高を引き出し、残り 120 円を現金で
    let mut s = Store::new(1, 2, 1);
    short_checkout(&mut s);
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"customer","choice":"cash"}),
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
        json!({"type":"resolve_emoney_short","register":1,"side":"customer","choice":"cash_confirmed"}),
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
            json!({"type":"back_to_registering","register":1,"side":"customer"})
        )
        .is_err(),
        "電子マネーで払った後は登録に戻れない"
    );
    ok(&mut s, json!({"type":"insert","register":1,"amount":1000}));
    ok(&mut s, json!({"type":"finish_insert","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.outlet, 880);
    assert_eq!(v.last_receipt.unwrap().method, "電子マネー・現金");

    // 案内を見て、やめることもできる（何も引き落とされない）
    let mut s = Store::new(1, 2, 1);
    short_checkout(&mut s);
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"customer","choice":"cash"}),
    );
    ok(
        &mut s,
        json!({"type":"resolve_emoney_short","register":1,"side":"customer","choice":"cancel"}),
    );
    let v = reg(&s, 1);
    assert_eq!(v.emoney_partial, None);
    assert_eq!(v.customer.method, None);
    assert_eq!(emoney_balance(&s), 1_000);

    // セミセルフではチャージを選べない
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

    // 有人モードのチャージ：入れてかざし、もう一度かざして払う
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

/// 取引中止はセミセルフのお客様もできる。入れたお金は出金ボックスに戻り、電子マネーで払った分は残高に戻る
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
    ok(&mut s, json!({"type":"insert","register":1,"amount":100}));
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    let v = reg(&s, 1);
    assert!(v.customer.lines.is_empty());
    assert_eq!(v.outlet, 100);
    assert_eq!(v.customer.inserted, 0);
}

/// 店員側・お客様側どちらの店舗メニューにも「ログアウト」と「レジ休止中」がある（お客様側はパスワードで入った後）。
/// ログアウトでセミセルフに戻る。休止中はスタッフバーコードを読むまでどの操作も受け付けない。
#[test]
fn logout_and_pause_from_either_store_menu() {
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    open_customer_menu(&mut s, 1);
    ok(&mut s, json!({"type":"end_staff_mode","register":1}));
    let v = reg(&s, 1);
    assert_eq!(v.mode, Mode::SemiSelf);
    assert!(v.menu.is_none());
    // 取引中は休止できない
    login(&mut s, 1);
    scan(&mut s, 1, "staff", BREAD);
    assert!(run(&mut s, json!({"type":"pause","register":1})).is_err());
    ok(&mut s, json!({"type":"void_transaction","register":1}));
    open_customer_menu(&mut s, 1);
    ok(&mut s, json!({"type":"pause","register":1}));
    let v = reg(&s, 1);
    assert!(v.paused);
    assert_eq!(v.mode, Mode::SemiSelf);
    assert!(v.menu.is_none());
    // 休止中はお客様が読んでも、呼び出しボタンを押しても受け付けない
    assert!(run(
        &mut s,
        json!({"type":"scan","register":1,"side":"customer","code":BREAD})
    )
    .is_err());
    assert!(run(&mut s, json!({"type":"call_staff","register":1})).is_err());
    // 店員側の店舗メニューからも休止できる。スタッフバーコードで再開して有人モードになる
    scan(&mut s, 1, "staff", "ST001");
    let v = reg(&s, 1);
    assert!(!v.paused);
    assert_eq!(v.mode, Mode::Attended);
    ok(
        &mut s,
        json!({"type":"open_menu","register":1,"screen":"top"}),
    );
    ok(&mut s, json!({"type":"pause","register":1}));
    assert!(reg(&s, 1).paused);
}

/// お渡し済みの FF の数量を増やすとお渡し前に戻り、減らす・取り消すと店員側に回収の案内が出る
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
    // 増やすとお渡し前に戻り、照合し直すまで会計に進めない
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
    advance(&mut s, 1_000);
    // 減らすと回収の案内（お渡し済みのまま）
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
    // 取り消しても回収の案内
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"remove_line","register":1,"side":"staff","line_id":id}),
    );
    assert_eq!(reg(&s, 1).staff.ff_collect[0].qty, 2);
}

/// 宅配便・公共料金・切手・たばこはポイント付与の対象外。付与は対象商品の金額だけから計算し、
/// 対象商品のない会計ではポイントカードの確認を出さない
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
    assert_eq!(reg(&s, 1).last_receipt.map(|r| r.points), Some(bread / 100));

    // 切手だけの会計では、ポイントカードの確認を出さない
    let mut s = Store::new(1, 2, 1);
    login(&mut s, 1);
    scan(&mut s, 1, "staff", STAMP);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
    assert!(!reg(&s, 1).staff.point_pending);
}

/// 有人モードの FF は相互チェック：スタッフ側の照合が済んでから、お客様側の「確認しました」を押せる。
/// 両方が済むまで会計に進めない。「確認しました」はお客様側の画面からだけ押せ、照合の結果は変えない。
/// 明細が増えたり個数が変わったりすると、お客様は確かめ直す
#[test]
fn customer_confirms_ff_receipt_in_attended_mode() {
    let mut s = Store::new(1, 1, 1);
    login(&mut s, 1);
    for code in ["FF01", "FF02"] {
        ok(
            &mut s,
            json!({"type":"register_ff","register":1,"side":"staff","code":code,"qty":1}),
        );
    }
    advance(&mut s, 1_000);
    let v = reg(&s, 1);
    let ids: Vec<u32> = v.staff.lines.iter().map(|l| l.line.id).collect();
    assert!(v.staff.ff_unconfirmed);
    // スタッフの照合の前は、お客様は確かめられない
    assert!(!allowed(&v, Side::Customer, Action::ReceiveFf));
    assert!(run(
        &mut s,
        json!({"type":"receive_ff","register":1,"side":"customer"})
    )
    .is_err());
    for &id in &ids {
        ok(
            &mut s,
            json!({"type":"check_ff","register":1,"side":"staff","line_id":id,"handed":1}),
        );
    }
    advance(&mut s, 1_000);
    // スタッフの照合だけでは会計に進めない
    let v = reg(&s, 1);
    assert!(!allowed(&v, Side::Staff, Action::Checkout));
    assert!(allowed(&v, Side::Customer, Action::ReceiveFf));
    assert!(run(
        &mut s,
        json!({"type":"receive_ff","register":1,"side":"staff"})
    )
    .is_err());
    ok(
        &mut s,
        json!({"type":"receive_ff","register":1,"side":"customer"}),
    );
    advance(&mut s, 1_000);
    let v = reg(&s, 1);
    assert!(!v.staff.ff_unconfirmed);
    assert!(
        !allowed(&v, Side::Customer, Action::ReceiveFf),
        "nothing left to confirm"
    );
    // FF を追加すると、照合も確かめもやり直し
    ok(
        &mut s,
        json!({"type":"register_ff","register":1,"side":"staff","code":"FF01","qty":1}),
    );
    advance(&mut s, 1_000);
    let v = reg(&s, 1);
    assert!(v.staff.ff_unconfirmed);
    assert!(!allowed(&v, Side::Staff, Action::Checkout));
    for l in v.staff.lines.iter().filter(|l| !ids.contains(&l.line.id)) {
        ok(
            &mut s,
            json!({"type":"check_ff","register":1,"side":"staff","line_id":l.line.id,"handed":l.line.qty}),
        );
    }
    advance(&mut s, 1_000);
    assert!(!allowed(&reg(&s, 1), Side::Staff, Action::Checkout));
    ok(
        &mut s,
        json!({"type":"receive_ff","register":1,"side":"customer"}),
    );
    advance(&mut s, 1_000);
    ok(
        &mut s,
        json!({"type":"checkout","register":1,"side":"staff"}),
    );
}
