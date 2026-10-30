/**
 * 第三階層：スタッフ側画面と顧客側画面の対表（仕様の正本）。
 *
 * 一つの「場面」（モード × 手続きの段階 × 出ている確認ごと）を一件として、両側の
 * 積極テキスト・消極テキスト・ボタン（左 → 右）を書く。読むための表 3-screens.md は
 * このデータから生成する（pos で `npx vitest run -u` を実行すると書き直す）。
 *
 * 書き方
 * - 〈 〉は値が入るところ。例：〈N〉点、〈金額〉
 * - ボタンは画面に並ぶ順（左 → 右、上の行 → 下の行）に書く
 * - rules は第二階層（2-principles.md）のルール番号、issues は下の ISSUES の番号
 */

export type Mode = "共通" | "有人" | "セミセルフ";

/** ボタンの種類。表示のみ＝押せない形で映すもの（有人モードの顧客側など） */
export type Tone = "積極" | "消極" | "長押し" | "表示のみ";

export interface Button {
  label: string;
  tone: Tone;
  /** 置き場所や出る条件など */
  note?: string;
}

export interface Face {
  /** 積極テキスト（濃い色・大きい文字） */
  strong?: string[];
  /** 消極テキスト（薄い色・小さい文字） */
  weak?: string[];
  /** ボタン（左 → 右、上の行 → 下の行） */
  buttons?: Button[];
  /** 表・入力欄・札など、テキストとボタン以外の部品 */
  parts?: string[];
  /** 反対側の画面と同じものを、押せない形で映す */
  mirror?: boolean;
  /** 本体を置き換えず、登録画面（明細）のまま */
  register?: boolean;
}

export interface Scene {
  id: string;
  mode: Mode;
  group: string;
  title: string;
  /** この場面が出る条件 */
  when: string;
  staff: Face;
  customer: Face;
  rules: string[];
  /** 描いている view.ts の関数 */
  source: string;
  issues?: string[];
}

export interface Issue {
  id: string;
  /** 照らすルール */
  rules: string[];
  text: string;
}

const stay: Face = { register: true };

export const SCENES: Scene[] = [
  // ---------------------------------------------------------------- 常設部分
  {
    id: "F-head",
    mode: "共通",
    group: "常設部分",
    title: "ヘッダー",
    when: "いつでも（レジ休止中はモードバッジを出さない）",
    staff: {
      parts: ["左：モードバッジ「有人モード▾」（有人：押すと店舗メニュー）または「セミセルフモード」", "中央：日付と時刻（分まで）"],
      buttons: [{ label: "🔔", tone: "長押し", note: "右。有人では仲間のスタッフを呼ぶ SOS。セミセルフではスタッフを呼ぶ" }],
    },
    customer: {
      parts: ["左：モードバッジ（有人：押すと店舗メニュー。パスワードが要る）", "中央：日付と時刻（分まで）"],
      buttons: [{ label: "🔔", tone: "長押し", note: "右。セミセルフでスタッフを呼ぶ。有人では押せない（スタッフが対応中です）" }],
    },
    rules: ["P-11"],
    source: "screenHead",
  },
  {
    id: "F-top-attended",
    mode: "有人",
    group: "常設部分",
    title: "上の帯（有人）",
    when: "明細が一つ以上あるとき",
    staff: {
      weak: ["ポイント：未読取／読取済（合計の上、中央）"],
      strong: ["合計 〈N〉点 〈金額〉（常に中央）"],
      buttons: [
        { label: "電子マネーのチャージ・残高確認", tone: "消極", note: "合計の下、中央。下線の付いた文字リンク" },
        { label: "取引中止", tone: "消極", note: "左端（引き返すボタンなので左）。縦幅は合計の文字の大きさ、横幅は会計と同じ" },
        { label: "会計", tone: "積極", note: "右端。会計中は出さない（引き返すボタンは F-back）" },
      ],
    },
    customer: {
      weak: ["ポイント：未読取／読取済（合計の上、中央）"],
      strong: ["合計 〈N〉点 〈金額〉（常に中央）"],
      buttons: [{ label: "投入おわり", tone: "積極", note: "右端。現金払いの間だけ。お金が足りるまで押せない" }],
    },
    rules: ["P-06", "P-10", "P-12", "P-19", "P-20"],
    source: "attendedScreen / regTop",
    issues: ["Q-02"],
  },
  {
    id: "F-top-semi",
    mode: "セミセルフ",
    group: "常設部分",
    title: "上の帯（セミセルフ）",
    when: "明細が一つ以上あるとき",
    staff: { mirror: true, buttons: [{ label: "（現金払いの間は右端に何も出さない）", tone: "表示のみ" }] },
    customer: {
      weak: ["ポイント：未読取／読取済（合計の上、中央）"],
      strong: ["合計 〈N〉点 〈金額〉（常に中央）"],
      buttons: [
        { label: "取引中止", tone: "消極", note: "左端（引き返すボタンなので左）。縦幅は合計の文字の大きさ、横幅は会計と同じ" },
        { label: "お会計", tone: "積極", note: "右端。お会計中は出さず、現金払いの間だけ「投入おわり」（積極）" },
      ],
    },
    rules: ["P-06", "P-12", "P-19", "P-20"],
    source: "semiSelfScreen / regTop",
    issues: ["Q-02", "Q-16"],
  },
  {
    id: "F-back",
    mode: "共通",
    group: "常設部分",
    title: "会計中の引き返すボタン",
    when: "会計ボタンの後（年確・ポイント・公共料金・決済手段など、会計中のどの確認画面でも）",
    staff: { buttons: [{ label: "商品登録に戻る", tone: "消極", note: "確認画面の左下。有人のスタッフ側。セミセルフでは顧客側と同じものを映す" }] },
    customer: { buttons: [{ label: "商品登録に戻る", tone: "消極", note: "確認画面の左下。セミセルフだけ（有人の顧客側には出さない）" }] },
    rules: ["P-05", "P-19"],
    source: "backFoot",
  },
  {
    id: "F-items-attended",
    mode: "有人",
    group: "常設部分",
    title: "最下段（有人）",
    when: "登録中で、確認画面・荷物の取引・店舗メニュー・電子マネーの手続きを出していないとき",
    staff: {
      buttons: [
        { label: "フライヤー／ドリンク／コールド／中華まん／おでん", tone: "消極", note: "タブ。選ばれたタブは積極" },
        { label: "FF の商品（残 〈N〉）", tone: "消極", note: "売り切れは無効" },
        { label: "▲ ▼", tone: "消極", note: "ページ送り。隠れたページがあるときだけ" },
      ],
    },
    customer: { parts: ["出さない"] },
    rules: ["P-05", "P-18"],
    source: "itemsPanel",
  },
  {
    id: "F-items-semi",
    mode: "セミセルフ",
    group: "常設部分",
    title: "最下段（セミセルフ）",
    when: "確認画面・店舗メニュー・電子マネーの手続きを出していないとき",
    staff: { mirror: true },
    customer: {
      buttons: [
        { label: "FF／揚げ物・中華まん", tone: "長押し", note: "スタッフを呼ぶ" },
        { label: "サービス／お支払い・荷物", tone: "長押し", note: "スタッフを呼ぶ" },
        { label: "たばこ", tone: "長押し", note: "スタッフを呼ぶ" },
      ],
    },
    rules: ["P-01"],
    source: "itemsPanel",
  },

  // ---------------------------------------------------------------- 共通
  {
    id: "C-paused",
    mode: "共通",
    group: "共通",
    title: "レジ休止中",
    when: "店舗メニューで「レジ休止中」を選んだ後",
    staff: { strong: ["レジ休止中"], weak: ["スタッフバーコードを読むと再開します"] },
    customer: { strong: ["レジ休止中"], weak: ["お手数ですが、ほかのレジをご利用ください"] },
    rules: ["P-07", "P-16"],
    source: "sharedScreen",
  },
  {
    id: "C-thanks",
    mode: "共通",
    group: "共通",
    title: "お支払いの後",
    when: "決済が済んでから 3 秒",
    staff: { strong: ["ありがとうございました"], weak: ["お忘れ物にご注意ください"] },
    customer: { strong: ["ありがとうございました"], weak: ["お忘れ物にご注意ください"] },
    rules: [],
    source: "thanks",
  },
  {
    id: "C-void",
    mode: "共通",
    group: "共通",
    title: "取引中止の確認",
    when: "「取引中止」を押した後（どの場面からでも。ほかのどの確認よりも先に出す）",
    staff: {
      strong: ["取引を中止しますか？"],
      weak: ["登録した 〈N〉点（合計 〈金額〉）をすべて取り消します。〈戻るお金の案内〉"],
      buttons: [
        { label: "いいえ", tone: "消極", note: "左。登録画面に戻る" },
        { label: "はい", tone: "積極", note: "右。取引を中止する" },
      ],
    },
    customer: {
      strong: ["有人：取引の中止を確認しています", "セミセルフ：取引を中止しますか？"],
      weak: ["セミセルフ：登録した 〈N〉点（合計 〈金額〉）をすべて取り消します。〈戻るお金の案内〉"],
      buttons: [
        { label: "いいえ", tone: "消極", note: "左。セミセルフだけ" },
        { label: "はい", tone: "積極", note: "右。セミセルフだけ" },
      ],
    },
    rules: ["P-07", "P-08", "P-09", "P-19", "P-20"],
    source: "voidConfirm",
  },

  // ---------------------------------------------------------------- 有人：待ち受けと登録
  {
    id: "A-welcome",
    mode: "有人",
    group: "有人：待ち受けと登録",
    title: "待ち受け",
    when: "明細もクーポンもないとき",
    staff: {
      strong: ["お辞儀とご挨拶をしましょう"],
      weak: ["ご用件をお伺いしてください", "（案内ボタンを押したら）〈読むバーコードの案内〉"],
      buttons: [
        { label: "お買い物", tone: "消極", note: "一段目。選ばれたものは積極" },
        { label: "宅配便", tone: "消極" },
        { label: "お支払い", tone: "消極" },
        { label: "電子マネー／チャージ・残高確認", tone: "消極", note: "二段目" },
      ],
    },
    customer: {
      strong: ["いらっしゃいませ"],
      weak: ["ご用件をお申し付けください"],
      buttons: [
        { label: "お買い物", tone: "表示のみ" },
        { label: "宅配便", tone: "表示のみ" },
        { label: "お支払い", tone: "表示のみ" },
        { label: "電子マネー／チャージ・残高確認", tone: "表示のみ" },
      ],
    },
    rules: ["P-01", "P-05", "P-16", "P-18"],
    source: "attendedWelcome",
  },
  {
    id: "A-register",
    mode: "有人",
    group: "有人：待ち受けと登録",
    title: "登録画面",
    when: "明細があり、確認を出していないとき",
    staff: {
      parts: ["スタッフ：〈名前〉／税の内訳", "明細（品目・数量・小計）", "FF の札「お渡し前」（押すとお渡しの確認）／「お渡し済」"],
      buttons: [
        { label: "−（数量 1 では削除）", tone: "消極", note: "数量の左。押すとその場で数量が変わる" },
        { label: "〈数量〉", tone: "積極", note: "押すとテンキーのポップアップが開き、新しい数量を打ち込む（やめる｜決定）" },
        { label: "＋", tone: "消極", note: "数量の右。サービス商品とクーポンは削除と数字だけ" },
      ],
    },
    customer: {
      parts: ["明細（品目・数量・小計）。数量は数字だけ", "FF の札「お渡し前」／「お渡し済」"],
    },
    rules: ["P-05", "P-13", "P-25"],
    source: "attendedScreen / staffLine / lineEditor",
    issues: ["Q-17"],
  },
  {
    id: "A-ff-verify",
    mode: "有人",
    group: "有人：待ち受けと登録",
    title: "FF のお渡しの確認",
    when: "スタッフが FF の札「お渡し前」を押した後",
    staff: {
      strong: ["お渡しの確認"],
      parts: ["表：品目｜注文数｜お渡し数（入力）｜照合（OK／NG）"],
      buttons: [{ label: "商品登録に戻る", tone: "消極", note: "左下" }],
    },
    customer: {
      strong: ["FF のお受け取りの確認"],
      parts: ["表：品目｜個数"],
      weak: ["スタッフがお渡しの個数を確認しています"],
      buttons: [{ label: "確認しました", tone: "積極", note: "右下。顧客側だけ。スタッフ側の照合が済むまで押せない" }],
    },
    rules: ["P-06", "P-07", "P-08", "P-19"],
    source: "ffVerifyScreen",
  },
  {
    id: "A-ff-receive",
    mode: "有人",
    group: "有人：待ち受けと登録",
    title: "FF のお受け取りの確認",
    when: "スタッフ側の照合がすべて OK になり、お客様がまだ「確認しました」を押していないとき。スタッフ側の会計ボタンはそれまで押せない（理由：お客様の FF のお受け取りの確認が済んでいません）",
    staff: stay,
    customer: {
      strong: ["FF のお受け取りの確認"],
      parts: ["表：品目｜個数"],
      weak: ["お受け取りになったら「確認しました」を押してください"],
      buttons: [{ label: "確認しました", tone: "積極", note: "右下。顧客側だけ。スタッフ側の照合と合わせて相互チェックになり、会計に進める" }],
    },
    rules: ["P-06", "P-07", "P-08", "P-21"],
    source: "ffReceiveCard",
  },

  // ---------------------------------------------------------------- 有人：荷物と公共料金
  {
    id: "A-pickup-find",
    mode: "有人",
    group: "有人：荷物と公共料金",
    title: "荷物を受取る：荷物を探す",
    when: "受取用バーコードを読んだ後、照合の前",
    staff: {
      weak: ["〈お問い合わせ番号〉［代金引換］"],
      strong: ["保管場所に荷物はありましたか？"],
      buttons: [
        { label: "無し", tone: "消極" },
        { label: "有り", tone: "積極" },
      ],
    },
    customer: { strong: ["荷物をお持ちします"], weak: ["少々お待ちください"] },
    rules: ["P-07", "P-08", "P-19"],
    source: "pickupStep",
  },
  {
    id: "A-pickup-missing",
    mode: "有人",
    group: "有人：荷物と公共料金",
    title: "荷物を受取る：見つからない",
    when: "「無し」を押した後",
    staff: {
      weak: ["〈お問い合わせ番号〉［見つかりません］"],
      strong: ["お客様にお詫びし、運送会社へお問い合わせください"],
      buttons: [
        { label: "探し直す", tone: "消極" },
        { label: "お受け取りを終える", tone: "積極" },
      ],
    },
    customer: { strong: ["申し訳ございません"], weak: ["お預かりの荷物が見つかりませんでした"] },
    rules: ["P-07", "P-19"],
    source: "pickupStep",
  },
  {
    id: "A-pickup-handover",
    mode: "有人",
    group: "有人：荷物と公共料金",
    title: "荷物を受取る：引き渡し",
    when: "代金引換の決済の後",
    staff: {
      weak: ["〈お問い合わせ番号〉"],
      strong: ["店舗控えにサインをいただき、荷物をお渡しください"],
      buttons: [{ label: "お渡ししました", tone: "積極" }],
    },
    customer: { strong: ["受け取りのサインをお願いします"], weak: ["レシートとお釣りをお受け取りください"] },
    rules: ["P-07"],
    source: "pickupHandover",
  },
  {
    id: "A-parcel-length",
    mode: "有人",
    group: "有人：荷物と公共料金",
    title: "荷物を送る：三辺の合計",
    when: "伝票を読んだ後、長さが決まる前",
    staff: {
      strong: ["（2 個目から〈N〉個目）取り扱いサイズを決定してください"],
      weak: ["三辺の長さの合計を入力してください（入力エラーのときはその理由）"],
      parts: ["入力欄：長さ 〈 〉cm"],
      buttons: [
        { label: "戻る", tone: "消極", note: "この荷物の受付をやめる" },
        { label: "決定", tone: "積極" },
      ],
    },
    customer: {
      strong: ["長さを計測しています"],
      weak: ["三辺の合計が200cm以内の荷物についてお取り扱いします（入力エラーのときはその理由）"],
    },
    rules: ["P-07", "P-08", "P-19"],
    source: "parcelStep / parcelNote",
  },
  {
    id: "A-parcel-weight",
    mode: "有人",
    group: "有人：荷物と公共料金",
    title: "荷物を送る：重さ",
    when: "長さが決まった後",
    staff: {
      strong: ["取り扱いサイズを決定してください"],
      weak: ["重量を入力してください"],
      parts: ["入力欄：重さ 〈 〉kg"],
      buttons: [
        { label: "戻る", tone: "消極" },
        { label: "決定", tone: "積極" },
      ],
    },
    customer: { strong: ["重さを計測しています"], weak: ["重さが30kg以内の荷物についてお取り扱いします"] },
    rules: ["P-07", "P-19"],
    source: "parcelStep / parcelNote",
  },
  {
    id: "A-parcel-postal",
    mode: "有人",
    group: "有人：荷物と公共料金",
    title: "荷物を送る：郵便番号",
    when: "サイズが決まった後",
    staff: {
      strong: ["お届け先の郵便番号を入力してください"],
      parts: ["入力欄：例 100-0001"],
      buttons: [
        { label: "戻る", tone: "消極" },
        { label: "決定", tone: "積極" },
      ],
    },
    customer: { strong: ["お届け先を入力しています"], weak: ["〈N〉サイズでお取り扱いします"] },
    rules: ["P-07", "P-19"],
    source: "parcelStep / parcelNote",
  },
  {
    id: "A-parcel-ask",
    mode: "有人",
    group: "有人：荷物と公共料金",
    title: "荷物を送る：次の荷物",
    when: "料金が決まった後",
    staff: {
      weak: ["〈N〉サイズ・〈地域〉あて（〒〈郵便番号〉）〈料金／着払い〉", "受付済み 〈N〉個"],
      strong: ["次の荷物はありますか？"],
      buttons: [
        { label: "ある", tone: "積極" },
        { label: "ない", tone: "積極" },
      ],
    },
    customer: { strong: ["ほかにお送りする荷物はございますか？"] },
    rules: ["P-07", "P-18", "P-19"],
    source: "parcelAsk",
  },
  {
    id: "A-bill-verify",
    mode: "有人",
    group: "有人：荷物と公共料金",
    title: "公共料金：払込票の確認",
    when: "会計ボタンの後、すべての払込票にチェックが入るまで",
    staff: {
      strong: ["受付 〈N〉枚 合計 〈金額〉"],
      parts: ["表：事業者｜金額｜バーコード番号｜確認（チェック）"],
      weak: ["事業者・金額・バーコード番号が合っていれば、一枚ずつチェックを入れてください"],
    },
    customer: { strong: ["公共料金〈N〉枚"], weak: ["スタッフが払込票を確認しています"] },
    rules: ["P-06", "P-07", "P-09"],
    source: "billVerifyScreen / billNote",
  },
  {
    id: "A-bill-pay",
    mode: "有人",
    group: "有人：荷物と公共料金",
    title: "公共料金：お支払い",
    when: "払込票の確認が済んだ後",
    staff: { strong: ["公共料金〈N〉枚"], weak: ["お客様に現金でお支払いいただいています"] },
    customer: { strong: ["公共料金〈N〉枚を受け付けます。"], weak: ["現金でお支払いください。"] },
    rules: ["P-07"],
    source: "billPayCard / billNote",
    issues: ["Q-06"],
  },
  {
    id: "A-bill-handover",
    mode: "有人",
    group: "有人：荷物と公共料金",
    title: "公共料金：お渡し",
    when: "公共料金の決済の後",
    staff: {
      strong: ["公共料金〈N〉枚"],
      weak: ["払込票に収納印を押してお渡しください"],
      buttons: [{ label: "お渡し済", tone: "積極" }],
    },
    customer: { strong: ["公共料金〈N〉枚　支払いが完了しました"], weak: ["払込票とレシート、お釣りをお受け取りください"] },
    rules: ["P-06", "P-07"],
    source: "billHandover",
  },

  // ---------------------------------------------------------------- 有人：会計
  {
    id: "A-age-choice",
    mode: "有人",
    group: "有人：会計",
    title: "年齢確認：するかどうか",
    when: "会計ボタンの後、年確が必要な商品があるとき",
    staff: {
      strong: ["〈X〉歳以上の年齢確認をしますか？"],
      buttons: [
        { label: "する", tone: "積極" },
        { label: "スキップ", tone: "消極" },
      ],
    },
    customer: { strong: ["年齢確認が必要な商品が含まれています"] },
    rules: ["P-06", "P-07", "P-09", "P-17", "P-19"],
    source: "idCard",
  },
  {
    id: "A-age-id",
    mode: "有人",
    group: "有人：会計",
    title: "年齢確認：生年月日",
    when: "「する」を押した後",
    staff: {
      strong: ["身分証の生年月日を入力してください"],
      parts: ["入力欄：西暦8桁　例 19821002"],
      buttons: [
        { label: "判定", tone: "積極" },
        { label: "スキップ", tone: "消極" },
      ],
    },
    customer: { strong: ["身分証を確認しています"] },
    rules: ["P-06", "P-07", "P-17", "P-19"],
    source: "idCard",
  },
  {
    id: "A-confiscated",
    mode: "有人",
    group: "有人：会計",
    title: "販売できない商品の回収",
    when: "年確で年齢に満たなかったとき",
    staff: {
      strong: ["販売できない商品について回収してください"],
      parts: ["チェックリスト：〈品名〉 × 〈N〉"],
      buttons: [{ label: "回収しました", tone: "積極", note: "全部にチェックを入れると押せる" }],
    },
    customer: { strong: ["販売できない商品があります"], weak: ["販売不可商品を回収いたします"] },
    rules: ["P-07", "P-09"],
    source: "confiscatedCard",
  },
  {
    id: "A-ff-collect",
    mode: "有人",
    group: "有人：会計",
    title: "お渡し済み FF の回収",
    when: "お渡し済みの FF を減らした・取り消した後",
    staff: {
      strong: ["〈品名〉を〈N〉点回収してください。"],
      buttons: [{ label: "回収しました", tone: "積極" }],
    },
    customer: {
      strong: ["取り消した FF をお返しください"],
      parts: ["一覧：〈品名〉 × 〈N〉"],
      weak: ["お返しになったら「確認しました」を押してください"],
      buttons: [{ label: "確認しました", tone: "積極", note: "右下。押すと回収の案内を閉じる" }],
    },
    rules: ["P-07", "P-09", "P-19"],
    source: "ffCollectCard",
    issues: ["Q-06"],
  },
  {
    id: "A-coupon-unmatched",
    mode: "有人",
    group: "有人：会計",
    title: "適用先のないクーポン",
    when: "会計に進んだとき、対象商品のないクーポンがある",
    staff: {
      strong: ["適用先のないクーポンがあります（太字）"],
      weak: ["対象商品と似た別の商品を通していないか確かめてください。対象商品を登録するか、クーポンを取り消すまで決済できません"],
      parts: ["クーポンの一覧"],
      buttons: [{ label: "取消", tone: "消極", note: "クーポンごと" }],
    },
    customer: {
      strong: ["ご利用になれないクーポンがあります（太字）"],
      weak: ["クーポンの対象商品とお買い上げの商品が同じか、お確かめください。対象商品を追加するか、クーポンを取り消すとお支払いに進めます"],
      parts: ["クーポンの一覧"],
    },
    rules: ["P-07", "P-09", "P-14"],
    source: "unmatchedCard",
    issues: ["Q-11"],
  },
  {
    id: "A-coupon-returned",
    mode: "有人",
    group: "有人：会計",
    title: "取り消したクーポンの返却",
    when: "クーポンを取り消した後",
    staff: {
      strong: ["取り消したクーポンをお客様に返却してください（太字）"],
      parts: ["クーポンの一覧"],
      buttons: [{ label: "返却しました", tone: "積極" }],
    },
    customer: {
      strong: ["取り消したクーポンをお返しします／適用先のなかったクーポンをお返しします（太字）"],
      parts: ["クーポンの一覧"],
    },
    rules: ["P-07", "P-09", "P-14"],
    source: "returnedCard",
    issues: ["Q-11"],
  },
  {
    id: "A-point",
    mode: "有人",
    group: "有人：会計",
    title: "ポイントカードの最終確認",
    when: "会計ボタンの後、ポイント対象の商品があり、まだ読んでいないとき",
    staff: {
      strong: ["ポイントを付与しますか？"],
      weak: ["（付与するを押したら）ポイントサービスのバーコードを読み取ってください。"],
      buttons: [
        { label: "付与する", tone: "積極" },
        { label: "スキップ", tone: "消極" },
      ],
    },
    customer: { strong: ["ポイントをお付けします"], parts: ["アイコン：ポイントカード・ポイントアプリ"] },
    rules: ["P-07", "P-09", "P-17", "P-19"],
    source: "pointCheck / customerPointAsk",
  },
  {
    id: "A-pay",
    mode: "有人",
    group: "有人：会計",
    title: "決済手段",
    when: "会計ボタンの後、ほかの確認が済んだとき",
    staff: {
      strong: ["決済手段をお伺いしてください（手段を選ぶかお金が入るまで）"],
      buttons: [
        { label: "現金", tone: "消極", note: "選んだ手段は積極。現金を入れた後は出さない" },
        { label: "電子マネー", tone: "消極" },
        { label: "バーコード決済", tone: "消極" },
        { label: "クレジットカード", tone: "消極" },
        { label: "ポイント利用", tone: "消極" },
      ],
      weak: ["〈支払いの案内〉例：お客様に現金を入れていただき、「投入おわり」を押していただいてください"],
    },
    customer: {
      strong: ["決済手段をお申しつけください（手段が決まるかお金が入るまで）"],
      buttons: [
        { label: "現金", tone: "表示のみ", note: "選ばれた手段を強調" },
        { label: "電子マネー", tone: "表示のみ" },
        { label: "バーコード決済", tone: "表示のみ" },
        { label: "クレジットカード", tone: "表示のみ" },
        { label: "ポイント利用", tone: "表示のみ" },
      ],
      weak: ["〈支払いの案内〉例：あと 〈金額〉 入れてください"],
    },
    rules: ["P-05", "P-07", "P-09", "P-10", "P-16", "P-18"],
    source: "attendedScreen / methodRow / methodList / payHint",
  },

  // ---------------------------------------------------------------- 電子マネー
  {
    id: "E-menu",
    mode: "共通",
    group: "電子マネー",
    title: "チャージか残高確認かを選ぶ",
    when: "電子マネーのボタン（待ち受け・上の帯）を押した後。セミセルフでは顧客側がこの「スタッフ側」の内容で操作し、スタッフ側は同じものを映す",
    staff: {
      strong: ["電子マネー"],
      weak: ["選択してください"],
      buttons: [
        { label: "チャージ", tone: "積極" },
        { label: "残高確認", tone: "積極" },
        { label: "やめる", tone: "消極", note: "左下" },
      ],
    },
    customer: {
      strong: ["電子マネー"],
      weak: ["選択してください"],
      buttons: [{ label: "（セミセルフではスタッフ側と同じボタン。有人では出さない）", tone: "表示のみ" }],
    },
    rules: ["P-05", "P-07", "P-19"],
    source: "emoneyScreen",
    issues: ["Q-08"],
  },
  {
    id: "E-charge",
    mode: "共通",
    group: "電子マネー",
    title: "チャージ",
    when: "「チャージ」を押した後。セミセルフでは顧客側がこの「スタッフ側」の内容で操作し、スタッフ側は同じものを映す",
    staff: {
      strong: ["チャージする紙幣を入れてください"],
      weak: ["入金ボックスに入れます。1,000円札から使えます（残高は 20,000円まで）／入れおわったら、電子マネーを入金ボックスにかざしてください"],
      buttons: [{ label: "やめる", tone: "消極", note: "左下" }],
    },
    customer: { mirror: true },
    rules: ["P-05", "P-19"],
    source: "emoneyScreen",
    issues: ["Q-08"],
  },
  {
    id: "E-inquiry",
    mode: "共通",
    group: "電子マネー",
    title: "残高確認",
    when: "「残高確認」を押した後。セミセルフでは顧客側がこの「スタッフ側」の内容で操作し、スタッフ側は同じものを映す",
    staff: {
      strong: ["電子マネーを入金ボックスにかざしてください → かざした後：〈金額〉"],
      weak: ["かざした後：電子マネーの残高（金額の上）"],
      buttons: [{ label: "やめる → かざした後：閉じる", tone: "消極", note: "左下" }],
    },
    customer: { mirror: true },
    rules: ["P-05", "P-19"],
    source: "emoneyScreen",
    issues: ["Q-08"],
  },
  {
    id: "E-short",
    mode: "共通",
    group: "電子マネー",
    title: "残高不足",
    when: "電子マネーで払おうとして残高が足りないとき。セミセルフでは顧客側がこの「スタッフ側」の内容で操作し、スタッフ側は同じものを映す",
    staff: {
      strong: ["電子マネーの残高が足りません"],
      weak: ["残高 〈金額〉・お支払い 〈金額〉"],
      buttons: [
        { label: "他の決済を使う", tone: "消極" },
        { label: "不足分を現金で払う", tone: "積極", note: "セミセルフでは右端" },
        { label: "電子マネーをチャージする", tone: "積極", note: "有人だけ" },
      ],
    },
    customer: { mirror: true },
    rules: ["P-01", "P-18", "P-19"],
    source: "emoneyScreen",
    issues: ["Q-08"],
  },
  {
    id: "E-partial",
    mode: "共通",
    group: "電子マネー",
    title: "不足分を現金で払う：確認",
    when: "「不足分を現金で払う」を押した後。セミセルフでは顧客側がこの「スタッフ側」の内容で操作し、スタッフ側は同じものを映す",
    staff: {
      strong: ["お支払い方法の確認 → 確認の後：電子マネーを入金ボックスにかざしてください"],
      weak: ["電子マネーの残高 〈金額〉 を引き出し、残り 〈金額〉 を現金でお支払いいただきます"],
      buttons: [
        { label: "他の決済を使う", tone: "消極", note: "左下" },
        { label: "確認しました", tone: "積極", note: "右下。確認の後は出さない" },
      ],
    },
    customer: { mirror: true },
    rules: ["P-19"],
    source: "emoneyScreen",
    issues: ["Q-08"],
  },

  // ---------------------------------------------------------------- 店舗メニュー
  {
    id: "M-staff",
    mode: "有人",
    group: "店舗メニュー",
    title: "スタッフ側から開いた店舗メニュー",
    when: "スタッフ側のモードバッジを押した後",
    staff: {
      strong: ["店舗メニュー（太字）"],
      weak: ["担当：〈名前〉"],
      buttons: [
        { label: "ジャーナル確認／廃棄登録／FF鮮度管理", tone: "積極", note: "準備中" },
        { label: "ログアウト／レジ休止中", tone: "積極" },
      ],
    },
    customer: stay,
    rules: ["P-01"],
    source: "storeMenu",
  },
  {
    id: "M-code",
    mode: "有人",
    group: "店舗メニュー",
    title: "お客様側から開く：パスワード",
    when: "顧客側のモードバッジを押した後",
    staff: {
      weak: ["お客様側の画面から店舗メニューを開こうとしています", "このパスワードをお客様側の画面で入力してください。一度きり有効です"],
      strong: ["〈4 桁のパスワード〉"],
    },
    customer: {
      strong: ["店舗メニューのパスワード"],
      weak: ["スタッフ側の画面に出ている 4 桁を入力してください"],
      parts: ["入力欄：例 U8X3"],
      buttons: [
        { label: "開く", tone: "積極", note: "入力欄の右" },
        { label: "やめる", tone: "消極", note: "左下" },
      ],
    },
    rules: ["P-07", "P-19"],
    source: "menuCode",
  },
  {
    id: "M-customer",
    mode: "有人",
    group: "店舗メニュー",
    title: "お客様側の店舗メニュー",
    when: "パスワードを入れた後",
    staff: { strong: ["お客様側の画面で店舗メニューを操作しています"] },
    customer: {
      strong: ["店舗メニュー（太字）"],
      weak: ["担当：〈名前〉"],
      buttons: [
        { label: "‹ 戻る", tone: "消極", note: "各項目の画面の左上" },
        { label: "返品返金／緊急出金／取引外入金／経費支出／自動釣銭機（両替）", tone: "積極", note: "取引中は押せない" },
        { label: "ログアウト／レジ休止中", tone: "積極" },
      ],
    },
    rules: ["P-07", "P-19"],
    source: "storeMenu",
  },

  // ---------------------------------------------------------------- セミセルフ
  {
    id: "S-welcome",
    mode: "セミセルフ",
    group: "セミセルフ",
    title: "待ち受け",
    when: "明細もクーポンもないとき",
    staff: { mirror: true },
    customer: {
      strong: ["バーコードをスキャンしてください"],
      weak: ["すべてのバーコードを受け付けます", "（案内ボタンを押したら）〈読むバーコードの案内〉"],
      buttons: [
        { label: "商品", tone: "消極", note: "選ばれたものは積極" },
        { label: "レジ袋", tone: "消極" },
        { label: "ポイント", tone: "消極" },
        { label: "お支払い", tone: "消極" },
        { label: "クーポン", tone: "消極" },
        { label: "電子マネーのチャージ・残高確認", tone: "消極", note: "右下の角の文字リンク" },
      ],
    },
    rules: ["P-01", "P-05", "P-18"],
    source: "welcome",
  },
  {
    id: "S-register",
    mode: "セミセルフ",
    group: "セミセルフ",
    title: "登録画面",
    when: "明細があり、確認を出していないとき",
    staff: { mirror: true },
    customer: {
      parts: ["税の内訳", "明細（品目・数量・小計）", "札：FF「お受け取り前／お受け取り済」、サービス「スタッフが受付します」"],
      buttons: [
        { label: "−（数量 1 では削除）", tone: "消極", note: "数量の左（有人と同じ）" },
        { label: "〈数量〉", tone: "積極", note: "押すとテンキーのポップアップが開く（有人と同じ）" },
        { label: "＋", tone: "消極", note: "数量の右。お渡し済みの FF は数字だけ（スタッフが変える）" },
      ],
    },
    rules: ["P-05", "P-13", "P-25"],
    source: "semiSelfScreen / lineEditor",
    issues: ["Q-17"],
  },
  {
    id: "S-call",
    mode: "セミセルフ",
    group: "セミセルフ",
    title: "スタッフの呼び出し中",
    when: "呼び出し（FF・サービス・たばこ・呼び鈴・年確などの自動呼び出し）が開いているとき",
    staff: {
      strong: ["🔔 〈呼び出しの理由〉（例：年確＆FF）"],
      weak: ["〈詳細〉", "〈N〉秒経過　スタッフバーコードでログインすると対応を始めます"],
      parts: ["Bell が不通のときは警告の帯"],
    },
    customer: stay,
    rules: ["P-05", "P-07"],
    source: "callScreen",
  },
  {
    id: "S-ff-verify",
    mode: "セミセルフ",
    group: "セミセルフ",
    title: "FF のお受け取りの確認",
    when: "お客様が FF の札「お受け取り前」を押した後",
    staff: { mirror: true },
    customer: {
      strong: ["お渡しの確認"],
      parts: ["表：品目｜注文数｜お受け取り数（入力）｜照合"],
      buttons: [{ label: "商品登録に戻る", tone: "消極", note: "左下" }],
    },
    rules: ["P-08", "P-16", "P-19"],
    source: "ffVerifyScreen",
    issues: ["Q-13"],
  },
  {
    id: "S-parcel-wait",
    mode: "セミセルフ",
    group: "セミセルフ",
    title: "荷物を送る：スタッフを待つ",
    when: "伝票を読んだ後、スタッフが受付するまで",
    staff: { mirror: true },
    customer: {
      strong: ["スタッフがお伺いします"],
      weak: ["伝票を貼り付けてお待ちください", "伝票の貼り付け、梱包はお客様責となります"],
    },
    rules: ["P-08", "P-16"],
    source: "parcelWait",
    issues: ["Q-14"],
  },
  {
    id: "S-bill-wait",
    mode: "セミセルフ",
    group: "セミセルフ",
    title: "公共料金：スタッフを待つ",
    when: "お会計の後、スタッフが払込票を確認するまで",
    staff: { mirror: true },
    customer: { strong: ["公共料金〈N〉枚"], weak: ["払込票の内容を確認いたします"] },
    rules: ["P-08", "P-09"],
    source: "billNote",
  },
  {
    id: "S-bill-pay",
    mode: "セミセルフ",
    group: "セミセルフ",
    title: "公共料金：お支払い",
    when: "スタッフの確認が済んだ後",
    staff: { mirror: true },
    customer: { strong: ["公共料金〈N〉枚を受け付けます。"], weak: ["現金でお支払いください。"] },
    rules: ["P-08"],
    source: "billPayCard",
    issues: ["Q-06"],
  },
  {
    id: "S-age-wait",
    mode: "セミセルフ",
    group: "セミセルフ",
    title: "年齢確認：スタッフを待つ",
    when: "お会計の後、年確が必要な商品がありスタッフがまだ来ていないとき",
    staff: { mirror: true },
    customer: { strong: ["年齢確認が必要な商品が含まれています。"], weak: ["〈X〉歳前後のお客様は身分証をご準備ください。"] },
    rules: ["P-08", "P-09"],
    source: "ageWait",
    issues: ["Q-06"],
  },
  {
    id: "S-coupon",
    mode: "セミセルフ",
    group: "セミセルフ",
    title: "クーポンの照合と返却",
    when: "適用先のないクーポンがあるとき／クーポンを取り消した後",
    staff: { mirror: true },
    customer: {
      strong: ["ご利用になれないクーポンがあります／取り消したクーポンをお返しします（太字）"],
      weak: ["クーポンの対象商品とお買い上げの商品が同じか、お確かめください。…"],
      parts: ["クーポンの一覧"],
      buttons: [
        { label: "取消", tone: "消極", note: "照合のとき、クーポンごと" },
        { label: "受け取りました", tone: "積極", note: "返却のとき" },
      ],
    },
    rules: ["P-09", "P-14"],
    source: "unmatchedCard / returnedCard",
    issues: ["Q-11"],
  },
  {
    id: "S-point",
    mode: "セミセルフ",
    group: "セミセルフ",
    title: "ポイントカードの最終確認",
    when: "お会計の後、ポイント対象の商品があり、まだ読んでいないとき",
    staff: { mirror: true },
    customer: {
      strong: ["ポイントを付与しますか？"],
      weak: ["（付与するを押したら）ポイントサービスのバーコードを読み取ってください。"],
      buttons: [
        { label: "付与する", tone: "積極" },
        { label: "スキップ", tone: "消極" },
      ],
    },
    rules: ["P-09", "P-16", "P-17", "P-19"],
    source: "pointCheck",
    issues: ["Q-12"],
  },
  {
    id: "S-pay",
    mode: "セミセルフ",
    group: "セミセルフ",
    title: "お支払い方法",
    when: "お会計の後、ほかの確認が済んだとき",
    staff: { mirror: true },
    customer: {
      buttons: [
        { label: "現金", tone: "消極", note: "選んだ手段は積極。現金を入れた後は出さない" },
        { label: "電子マネー", tone: "消極" },
        { label: "バーコード決済", tone: "消極" },
        { label: "クレジットカード", tone: "消極" },
        { label: "ポイント利用", tone: "消極" },
      ],
      weak: ["〈支払いの案内〉例：現金を入金ボックスに入れるか、支払方法を選んでください"],
    },
    rules: ["P-09", "P-10", "P-18"],
    source: "semiSelfScreen / methodRow / payHint",
  },
  {
    id: "S-bill-handover",
    mode: "セミセルフ",
    group: "セミセルフ",
    title: "公共料金：お渡し",
    when: "公共料金の決済の後",
    staff: {
      strong: ["公共料金〈N〉枚"],
      weak: ["払込票に収納印を押してお渡しください"],
      buttons: [{ label: "お渡し済", tone: "積極" }],
    },
    customer: { strong: ["公共料金〈N〉枚　支払いが完了しました"], weak: ["払込票とレシート、お釣りをお受け取りください"] },
    rules: ["P-06", "P-07"],
    source: "billHandover",
  },
];

/** 棚卸しで見つかった、判断が要る点 */
export const ISSUES: Issue[] = [
  { id: "Q-02", rules: ["P-16"], text: "会計ボタンの名前が、有人は「会計」、セミセルフは「お会計」。スタッフ向けとお客様向けの使い分けなら、P-16 に書き足す。" },
  { id: "Q-06", rules: ["P-14"], text: "積極テキストの句点がそろっていない。「年齢確認が必要な商品が含まれています。」「公共料金〈N〉枚を受け付けます。」「〈品名〉を〈N〉点回収してください。」は「。」で終わり、ほかは「。」なし。" },
  { id: "Q-08", rules: ["P-07", "P-16"], text: "有人の電子マネーの手続きでは、顧客側にスタッフ側と同じ文言（「選択してください」など）がボタンなしで出る。対のメッセージにしなくてよいか。" },
  { id: "Q-11", rules: ["P-08", "P-14"], text: "クーポンの照合・返却の画面だけ形が違う。見出しが大きい文字ではなく太字で、ボタンが見出しの横に小さく出る。" },
  { id: "Q-12", rules: ["P-16"], text: "セミセルフのポイントの最終確認で、お客様に「ポイントを付与しますか？」「付与する」とスタッフ向けの言葉を出している。" },
  { id: "Q-13", rules: ["P-16"], text: "セミセルフの FF の確認で、列は「お受け取り数」なのに見出しは「お渡しの確認」。" },
  { id: "Q-14", rules: ["P-16"], text: "「伝票の貼り付け、梱包はお客様責となります」の「お客様責」は「お客様の責任」の誤記か。" },
  { id: "Q-16", rules: [], text: "（コードの整理）セミセルフの上の帯の電子マネーボタンは、明細がないときだけ出す作りだが、明細がないと上の帯そのものが出ないので、表示されることがない。" },
  { id: "Q-17", rules: ["P-18"], text: "切り替えボタンの列（決済手段・FF のタブ・待ち受けの案内ボタン）は、選ばれていない側が消極で並ぶ。これは例外としてよいか。（明細の行を開いたときの並びは P-18 の対で解決済み）" },
];
