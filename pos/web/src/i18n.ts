
export type Lang = "ja" | "en";
export const DEFAULT_LANG: Lang = "ja";

const langs = new Map<string, Lang>();

export function langOf(reg: number, side: string): Lang {
  return langs.get(`${reg}:${side}`) ?? DEFAULT_LANG;
}

export function toggleLang(reg: number, side: string): Lang {
  const next: Lang = langOf(reg, side) === "ja" ? "en" : "ja";
  langs.set(`${reg}:${side}`, next);
  return next;
}

export function resetLangs(reg: number): void {
  langs.delete(`${reg}:staff`);
  langs.delete(`${reg}:customer`);
}

const JP = /[　-ヿ一-鿿！-～]/;
const YEN = /(-|−)?([\d,]+(?:\.\d+)?)\s?円/g;

const yenify = (s: string): string => s.replace(YEN, (_, minus: string | undefined, n: string) => `${minus ?? ""}¥${n}`);
const plural = (n: string | number, one: string, many = `${one}s`): string => `${n} ${Number(String(n).replace(/,/g, "")) === 1 ? one : many}`;

const WEEKDAY: Record<string, string> = { 日: "Sun", 月: "Mon", 火: "Tue", 水: "Wed", 木: "Thu", 金: "Fri", 土: "Sat" };
const MONTH = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

const DICT: Record<string, string> = {
  // 共通の部品
  バック: "Back",
  スキップ: "Skip",
  決定: "OK",
  閉じる: "Close",
  はい: "Yes",
  する: "Check",
  判定: "Verify",
  削除: "Delete",
  取消: "Cancel",
  "＋": "+",
  上限: "Max",
  確認: "Check",
  照合: "Match",
  品目: "Item",
  数量: "Qty",
  小計: "Amount",
  金額: "Amount",
  合計: "Total",
  注文数: "Ordered",
  "・": " / ",
  "—": "—",

  // ヘッダー
  スタッフ操作モード: "Staff Mode",
  セルフ操作モード: "Self Mode",
  店舗処理モード: "Store Mode",
  呼び出し中: "Calling staff",
  "仲間のスタッフを呼ぶ（押している間鳴ります）": "Call a colleague (rings while pressed)",
  "スタッフを呼ぶ（押している間鳴ります）": "Call staff (rings while pressed)",

  // 待ち受け
  電子マネー: "E-money",
  "チャージ・残高照会": "Top-up / Balance",
  バーコードをスキャンしてください: "Please scan a barcode",
  すべてのバーコードを受け付けます: "Any barcode is accepted",
  商品: "Items",
  レジ袋: "Bags",
  ポイント: "Points",
  お支払い: "Bills",
  クーポン: "Coupons",
  商品のバーコードをスキャンしてください: "Please scan the item's barcode",
  レジ袋のバーコードをスキャンしてください: "Please scan the bag's barcode",
  ポイントカードまたはポイントアプリのバーコードをスキャンしてください: "Please scan your point card or point app barcode",
  支払い用紙のバーコードをスキャンしてください: "Please scan the payment slip's barcode",
  "引換券、割引券のバーコードをスキャンしてください": "Please scan the voucher or discount coupon barcode",
  お買い物: "Shopping",
  宅配便: "Parcels",
  "宅配便の伝票、または受取用のバーコードをスキャンしてください": "Scan the parcel slip or the pickup barcode",
  いらっしゃいませ: "Welcome",
  ご用件をお申し付けください: "How may we help you?",
  お辞儀とご挨拶をしましょう: "Bow and greet the customer",
  ご用件をお伺いしてください: "Ask how you can help",
  休止中: "Closed",
  空いているレジをお使い下さい: "Please use another register",
  レジ休止中: "Register closed",
  "お手数ですが、ほかのレジをご利用ください": "Sorry, please use another register",
  スタッフバーコードを読むと再開します: "Scan a staff barcode to reopen",

  // 第一待ち受け
  有人レジ: "Staffed",
  セルフレジ: "Self",

  // 登録画面
  会計: "Pay",
  お会計: "Pay",
  取引中止: "Cancel",
  "取引を中止する（確認があります）": "Cancel the transaction (asks to confirm)",
  明細はありません: "No items yet",
  どのバーコードからでも読めます: "Any barcode can be scanned",
  "スタッフ：": "Staff: ",
  "（オーナー）": " (Owner)",
  内税: "Tax incl.",
  現金のみ: "Cash only",
  お渡し前: "To hand over",
  お渡し済: "Handed over",
  年確済: "Age checked",
  お受け取り前: "Not received",
  お受け取り済: "Received",
  押してお渡しした個数を照合: "Press to check the handed-over count",
  スタッフが受付します: "Staff will handle this",
  受付: "Accept",
  受付済: "Accepted",
  受付待ち: "Waiting",
  受付中: "In progress",
  対象商品待ち: "Awaiting item",
  照合済: "Matched",
  照合待ち: "To match",
  見つかりません: "Not found",
  代金引換: "Cash on delivery",
  着払い: "Collect",
  "ポイント：読取済": "Points: card read",
  "ポイント：未読取": "Points: not read",
  新しい数量: "New quantity",

  // FF
  フライヤー: "Fryer",
  ドリンク: "Drinks",
  コールド: "Cold",
  中華まん: "Steamed buns",
  おでん: "Oden",
  前のページ: "Previous page",
  次のページ: "Next page",
  "欠品：長押しで予約注文として登録します": "Sold out: long-press to register as a pre-order",
  "揚げ物・中華まん": "Fried food / Buns",
  サービス: "Service",
  "お支払い・荷物": "Bills / Parcels",
  金券: "Vouchers",
  "ギフトカード・印紙": "Gift cards / Revenue stamps",
  たばこ: "Tobacco",
  "紙巻き・電子": "Cigarettes / E-cigs",
  お渡しの確認: "Confirm hand-over",
  お渡し数: "Handed",
  お受け取り数: "Received",

  // 取引中止
  "取引を中止しますか？": "Cancel this transaction?",
  取引の中止を確認しています: "Confirming the cancellation",

  // 支払い
  現金: "Cash",
  バーコード決済: "Barcode pay",
  クレジットカード: "Credit card",
  ポイント利用: "Use points",
  決済手段をお伺いしてください: "Ask how the customer will pay",
  決済手段をお申しつけください: "Please tell us how you'd like to pay",
  お支払い方法を選択してください: "Please choose a payment method",
  お支払い方法をお申しつけください: "Please tell us how you'd like to pay",
  "お預かりした現金を入金ボックスに入れるか、支払方法を選んでください": "Put the received cash in the inlet or choose a payment method",
  "お預かりした現金を入金ボックスに入れ、「投入おわり」を押してください": "Put the received cash in the inlet and press \"Done\"",
  スタッフが現金をお預かりします: "Staff will take your cash",
  "現金のみの商品（切手・チケット・お支払い）は現金でお支払いください": "Please pay for cash-only items (stamps, tickets, bills) in cash",
  投入おわり: "Done",
  お金を入れてください: "Please insert money",
  ありがとうございました: "Thank you",
  お忘れ物にご注意ください: "Please don't forget your belongings",
  レシートをお受け取りください: "and your receipt",
  現金はスタッフがお預かりいたします: "Staff will take your cash",
  現金は店員がお預かりいたします: "Staff will take your cash",
  店員が向かっています: "A staff member is on the way",
  スタッフが向かっています: "A staff member is on the way",
  釣り銭が残っています: "Change is left in the tray",
  呼び出し端末が動作していません: "The call system is not working",
  "お手数ですが、店員に直接お声かけください": "Sorry for the trouble, please speak to a staff member directly",
  スタッフの応答がありません: "No response from staff",
  "警察：": "Police: ",
  "消防救急：": "Fire / ambulance: ",
  事件または事故の可能性があります: "There may have been an incident or accident",
  "ご自身の安全を確保の上、通報をお願いいたします": "Please make sure you are safe, then call for help",
  スタッフをお呼びしました: "We have called a staff member",
  そのままお待ちください: "Please wait a moment",

  // ポイント
  "ポイントを付与しますか？": "Add points?",
  付与する: "Add",
  ポイントサービスのバーコードを読み取ってください: "Scan the point service barcode",
  ポイントをお付けします: "We'll add points",
  ポイントカード: "Point card",
  ポイントアプリ: "Point app",

  // 年齢確認
  身分証の生年月日を入力してください: "Enter the date of birth on the ID",
  "西暦8桁 例 19821002": "8 digits, e.g. 19821002",
  "身分証の生年月日（西暦8桁）": "Date of birth on ID (8 digits)",
  年齢確認が必要な商品が含まれています: "Some items require age verification",
  身分証を確認しています: "Checking ID",
  販売できない商品があります: "Some items cannot be sold",
  販売不可商品を回収いたします: "We will take back the items we cannot sell",
  販売できない商品について回収してください: "Take back the items that cannot be sold",
  回収しました: "Taken back",
  回収した商品にチェックを入れてください: "Tick the items you took back",

  // クーポン
  適用先のないクーポンがあります: "A coupon has no matching item",
  対象商品と似た別の商品を通していないか確かめてください: "Check that a similar but different item wasn't scanned",
  "対象商品を登録するか、クーポンを取り消すまで決済できません": "Payment is blocked until the item is added or the coupon is cancelled",
  ご利用になれないクーポンがあります: "A coupon cannot be used",
  "クーポンの対象商品とお買い上げの商品が同じか、お確かめください": "Please check that your items match the coupon",
  "対象商品を追加するか、クーポンを取り消すとお支払いに進めます": "Add the item or cancel the coupon to continue to payment",
  取り消したクーポンをお客様に返却してください: "Return the cancelled coupon to the customer",
  適用先のなかったクーポンをお返しします: "Here is your unused coupon back",
  取り消したクーポンをお返しします: "Here is your cancelled coupon back",
  返却しました: "Returned",
  受け取りました: "Received",

  // 電子マネー
  選択してください: "Please choose",
  チャージ: "Top-up",
  残高確認: "Balance",
  電子マネーを決済端末にかざしてください: "Hold your e-money card over the payment terminal",
  バーコードを決済端末にかざしてください: "Hold the barcode over the payment terminal",
  クレジットカードを決済端末にかざしてください: "Hold your credit card over the payment terminal",
  ポイントカードを決済端末にかざしてください: "Hold your point card over the payment terminal",
  ポイントアプリもお使いいただけます: "A point app also works",
  お客様に決済端末の位置を案内してください: "Show the customer where the payment terminal is",
  決済用バーコードをスキャンしてください: "Scan the customer's payment barcode",
  決済用バーコードをご提示ください: "Please show your payment barcode",
  電子マネーの残高: "E-money balance",
  チャージする紙幣を入れてください: "Insert the bills to top up",
  "入れおわったら、電子マネーを決済端末にかざしてください": "When done, hold your e-money card over the payment terminal",
  入金ボックスに入れます: "Put them in the inlet",
  電子マネーの残高が足りません: "Not enough e-money balance",
  不足額: "Short by",
  "不足額を\n現金で払う": "Pay the rest\nin cash",
  "電子マネーを\nチャージする": "Top up\ne-money",
  他の決済を使う: "Use another method",
  お支払い方法の確認: "Confirm payment method",
  確認しました: "Confirmed",

  // チケット
  チケット: "Ticket",
  チケットの発券待ち: "Waiting to issue tickets",
  "チケットのお渡しが済むまで、次の取引は始められません": "The next sale can't start until the tickets are handed over",
  ほかのレジのチケットのお渡しが済むと発券します: "Tickets will print once another register finishes handing over its tickets",
  発券の順番をお待ちください: "Please wait for your tickets to print",
  "チケットは一件ずつのお取り扱いです（ほかのチケットや商品と一緒に会計できません）": "Tickets are handled one order at a time (not with other tickets or items)",
  受け取り枚数をご入力ください: "Please enter the number of tickets you received",
  受け取り枚数: "Tickets received",
  発券枚数: "Tickets",
  お渡し枚数を入力してください: "Enter the number of tickets handed over",
  お渡し枚数: "Tickets handed over",
  発券枚数とお渡し枚数が一致しません: "The number handed over doesn't match the number issued",
  "発券枚数とお渡し枚数が一致しません\n再度ご確認ください": "The number handed over doesn't match the number issued\nPlease check again",
  お渡し枚数を入力しました: "Number handed over entered",
  受け取り枚数を入力しました: "Number received entered",
  チケットのお渡し枚数はスタッフ側の画面で入力します: "Staff enter the number of tickets handed over on the staff screen",
  チケットのお渡し枚数はスタッフがログインしてから入力します: "Staff enter the number of tickets handed over after logging in",
  申込数と受け取り数が一致しません: "The number received doesn't match the number ordered",
  再度ご確認ください: "Please check again",
  "申込数と受け取り数が一致しません\n再度ご確認ください": "The number received doesn't match the number ordered\nPlease check again",
  "チケットの受け取り枚数はお客様側の画面で入力していただきます": "The customer enters the number of tickets on the customer screen",
  お渡しするチケットはありません: "No tickets to hand over",
  "申込券でチケットを買う、現金のみで一件ずつ会計、支払い後に発券し、スタッフのお渡し枚数とお客様の受け取り枚数をそれぞれ発券枚数と照合する":
    "Buy tickets with an application slip; cash only, one order per sale; after payment the tickets print, and the staff count handed over and the customer count received are each checked against the number issued",

  // 公共料金
  事業者: "Payee",
  バーコード番号: "Barcode no.",
  "事業者・金額・バーコード番号が合っていれば、一枚ずつチェックを入れてください": "Tick each slip whose payee, amount and barcode number match",
  現金はスタッフがお預かりします: "Staff will take your cash",
  スタッフをお呼びください: "Please call a staff member",
  現金でお支払いください: "Please pay in cash",
  "払込票とレシート、お釣りをお受け取りください": "Please take your slips, receipt and change",
  払込票に収納印を押してお渡しください: "Stamp the slips and hand them over",
  スタッフが払込票を確認しています: "Staff is checking your slips",
  "お客様に受付点数、合計金額をお伝えください": "Tell the customer the number of slips and the total",
  払込票の内容を確認いたします: "Staff will check your slips",
  印紙貼付前: "Stamp needed",
  印紙: "Stamp",
  たばこカートン: "Cigarette carton",
  花火: "Fireworks",
  化粧品: "Cosmetics",
  レシート: "Receipt",
  "金額（税抜）": "Amount (excl. tax)",
  "レシートに収入印紙を貼り付けたら、チェックを入れてください": "Tick once the receipt has a revenue stamp",
  レシートに収入印紙を貼り付けています: "Affixing a revenue stamp to your receipt",
  レシートに収入印紙を貼り付けます: "A revenue stamp will be affixed to your receipt",
  貼付: "Affixed",
  収入印紙の貼り付け確認: "Confirm revenue stamps",
  "領収証に収入印紙を貼り付けたら、一枚ずつチェックを入れてください": "Tick each slip once its receipt has a revenue stamp",
  スタッフが領収証に収入印紙を貼り付けています: "Staff is affixing revenue stamps to your receipts",
  スタッフが領収証に収入印紙を貼り付けます: "Staff will affix revenue stamps to your receipts",
  収入印紙の貼り付けが確認できていません: "Revenue stamps have not been confirmed",

  // 販売期限・日配食品
  期限切れ: "Expired",
  販売期限: "Sell by",
  賞味期限: "Best before",
  品名: "Item",
  販売期限が切れています: "This item is past its sell-by date",
  "お客様にお詫びし、販売期限切れ商品を回収してください": "Apologise and collect the expired item",
  "申し訳ございません、店員が新しい商品にお取替えいたします": "Sorry, a staff member will swap it for a fresh one",
  "販売期限切れの商品があります\n取り消してから、お会計に進んでください": "An item is past its sell-by date\nCancel it before checking out",
  日配食品は一点ずつスキャンしてください: "Scan fresh food one at a time",
  "日配食品は数量を増やせません\n一点ずつスキャンしてください": "Fresh food quantity can't be raised\nScan each one",
  押して新しい商品との取り替えを案内: "Press to guide the swap for a fresh item",
  公共料金: "Utility bills",

  // 荷物
  "保管場所に荷物はありましたか？": "Was the parcel in storage?",
  無し: "No",
  有り: "Yes",
  探し直す: "Search again",
  お受け取りを終える: "End pickup",
  "お客様にお詫びし、運送会社へお問い合わせください": "Apologize to the customer and contact the carrier",
  受け取りのサインをお願いします: "Please sign for the parcel",
  レシートとお釣りをお受け取りください: "Please take your receipt and change",
  "店舗控えにサインをいただき、荷物をお渡しください": "Get a signature on the store copy and hand over the parcel",
  お渡ししました: "Handed over",
  申し訳ございません: "We're sorry",
  お預かりの荷物が見つかりませんでした: "We couldn't find your parcel",
  荷物をお持ちします: "We'll bring your parcel",
  少々お待ちください: "Please wait a moment",
  "ほかにお送りする荷物はございますか？": "Do you have another parcel to send?",
  お届け先を入力しています: "Entering the destination",
  重さを計測しています: "Weighing the parcel",
  重さが30kg以内の荷物についてお取り扱いします: "We accept parcels up to 30 kg",
  長さを計測しています: "Measuring the parcel",
  三辺の合計が200cm以内の荷物についてお取り扱いします: "We accept parcels up to 200 cm (length + width + height)",
  スタッフがお伺いします: "A staff member will help you",
  伝票を貼り付けてお待ちください: "Please attach the shipping slip and wait",
  "伝票の貼り付け、梱包はお客様責となります": "Attaching the slip and packing are your responsibility",
  三辺の長さの合計を入力してください: "Enter the sum of the three sides",
  重量を入力してください: "Enter the weight",
  "荷物の三辺の合計（cm）": "Sum of the parcel's three sides (cm)",
  "荷物の重さ（kg）": "Parcel weight (kg)",
  "例 100-0001": "e.g. 100-0001",
  "お届け先の郵便番号（7桁）": "Destination postal code (7 digits)",
  "次の荷物はありますか？": "Another parcel?",
  ある: "Yes",
  ない: "No",
  三辺の長さの合計: "Sum of the three sides",
  荷物の重さ: "Parcel weight",
  長さを入力してください: "Enter the length",
  重さを入力してください: "Enter the weight",
  三辺の合計を入力し直してください: "Re-enter the sum of the three sides",
  重さを入力し直してください: "Re-enter the weight",
  先に取り扱いサイズを決めてください: "Decide the size first",
  "郵便番号は 7 桁の数字で入力してください": "Enter a 7-digit postal code",
  この郵便番号の地域が見つかりません: "No region found for this postal code",
  "長さ": "Length",
  "重さ": "Weight",
  北海道: "Hokkaido",
  北東北: "North Tohoku",
  南東北: "South Tohoku",
  関東: "Kanto",
  信越: "Shinetsu",
  北陸: "Hokuriku",
  中部: "Chubu",
  関西: "Kansai",
  中国: "Chugoku",
  四国: "Shikoku",
  九州: "Kyushu",
  沖縄: "Okinawa",

  // テンキー
  入金: "Deposit",
  出金する金額: "Amount to withdraw",
  店舗パスワード: "Store password",
  "金額（円）": "Amount (yen)",
  円: "yen",

  // 呼び出し（セルフ操作モードのスタッフ側）
  煙草: "Tobacco",
  呼び出し: "Call",
  年確: "Age check",
  釣銭忘れ: "Change left",
  年齢確認: "Age check",
  "FF の注文": "FF order",
  "FF のご注文": "FF order",
  サービス受付: "Service",
  お呼び出し: "Call",
  お客様からの呼び出し: "Customer call",
  "チャージ・残高確認": "Top-up / Balance",
  スタッフバーコードでログインすると対応を始めます: "Log in with a staff barcode to respond",

  // 店舗メニュー
  店舗メニュー: "Store Menu",
  メニュー: "Menu",
  返品返金: "Refund",
  緊急出金: "Emergency withdrawal",
  取引外入金: "Non-sale deposit",
  経費支出: "Expense",
  "自動釣銭機（両替）": "Change machine (exchange)",
  ジャーナル確認: "Journal",
  廃棄登録: "Waste",
  FF鮮度管理: "FF freshness",
  ログアウト: "Log out",
  "準備中です（今後つくりこみます）": "Coming soon",
  お金を抜く操作です: "This takes money out",
  操作のたびに店舗パスワードが要ります: "The store password is required every time",
  "お金は下の「出金」ボックスに出ます": "The money comes out of the outlet below",
  記録は担当者名つきで残ります: "It is logged with the staff name",
  出金: "Withdraw",
  金額と店舗パスワードを入れてください: "Enter the amount and the store password",
  入金する金額を下の: "Put the amount to deposit into the ",
  両替する金額を: "Put the amount to exchange into the ",
  "「入金」ボックス": "inlet",
  "「出金」ボックス": "outlet",
  "に入れて Enter を押してください": " and press Enter",
  に出します: "",
  受け取る金種: "Receive in",

  // 店舗パスワード・認証の主な理由
  "お会計前です": "Not at checkout yet",
  "お会計中です": "Checkout in progress",
  "すでにお会計中です": "Already at checkout",
  "すでに会計画面です": "Already at checkout",
  明細がありません: "No items",
  商品を登録してください: "Please register items",
  会計画面ではありません: "Not at checkout",
  お会計中ではありません: "Not at checkout",
  クーポンはありません: "No coupons",
  中止する取引はありません: "No transaction to cancel",
  もう一方の画面で操作中です: "The other screen is in use",
  もう一方の画面で店舗メニューを操作中です: "The store menu is open on the other screen",
  "FF はタブから登録します": "Register FF from the tabs",
  "FF はスタッフにお申し付けください": "Please ask staff for FF",
  "FF のお渡し確認はスタッフ側の画面で行います": "FF hand-over is confirmed on the staff screen",
  "FF のお渡し確認はスタッフがログインしてから行います": "FF hand-over is confirmed after staff log in",
  "FF のお渡し確認（お渡し個数の照合）が済んでいません": "FF hand-over count has not been checked",
  "FF の明細がありません": "No FF items",
  お会計中はお渡し確認を変えられません: "Hand-over cannot be changed during checkout",
  お会計中は会計の流れで年齢確認をします: "During checkout, the age check is part of checkout",
  年齢確認の済んでいない商品はありません: "No items need an age check",
  年齢確認はスタッフ側の画面で行います: "Age check is done on the staff screen",
  年齢確認はスタッフがログインしてから行います: "Age check is done after staff log in",
  払込票の確認はスタッフがログインしてから行います: "Slips are checked after staff log in",
  "払込票の確認・お渡しはスタッフ側の画面で行います": "Slips are checked and handed over on the staff screen",
  お客様側の画面で押していただきます: "The customer presses this on their screen",
  "スタッフの操作です（スタッフバーコードでログインしてください）": "Staff operation (log in with a staff barcode)",
  スタッフバーコードでログインしてください: "Log in with a staff barcode",
  スタッフが年齢確認をするか選んでいません: "Staff has not chosen whether to check age",
  スタッフが身分証の生年月日を入力していません: "Staff has not entered the date of birth",
  サービス商品の受付が済んでいません: "A service item is not processed yet",
  スタッフがサービス商品の受付をするまでお待ちください: "Please wait until staff processes the service item",
  次の荷物があるか選んでください: "Choose whether there is another parcel",
  ポイントカードの確認が済んでいません: "Point card has not been checked",
  ポイントカードの確認は済んでいます: "Point card already checked",
  スタッフが公共料金の内容を確認していません: "Staff has not checked the bills",
  "切手・チケット・お支払いは現金のみです": "Stamps, tickets and bills are cash only",
  スタッフがお預かりします: "Staff will take your cash",
  セルフレジでは現金を使えません: "Cash can't be used at self checkout",
  "ほかの支払方法を選ぶか、スタッフをお呼びください": "Choose another method or call a staff member",
  どうするか選んでください: "Please choose what to do",
  "対象商品を確かめるか、クーポンを取り消してください": "Check the item or cancel the coupon",
  現金以外のお支払いが選ばれています: "A non-cash method is selected",
  キャッシュレスの支払方法が選ばれていません: "No cashless method is selected",
  "取引中は休止できません（取引を終えるか中止してください）": "Can't close during a transaction (finish or cancel it)",
  取引中は出金操作ができません: "Can't withdraw during a transaction",
  取引中は入金操作ができません: "Can't deposit during a transaction",
  受付待ちのサービス商品はありません: "No service item waiting",
  次の荷物をお尋ねする場面ではありません: "Not asking about another parcel now",
  年齢確認は不要です: "No age check needed",
  先に年齢確認をするか選んでください: "Choose whether to check age first",
  生年月日の入力は不要です: "No date of birth needed",
  先に年齢確認をしてください: "Check age first",
  "買い物点数の上限（100点）に達しています": "The item limit (100) has been reached",
  公共料金の払込票はありません: "No utility bill slips",
  会計ボタンを押してから確認します: "Checked after pressing Pay",
  お渡しする払込票はありません: "No slips to hand over",
  電子マネーの残高は足りています: "E-money balance is sufficient",
  "お買い物の途中は電子マネーのチャージ・残高確認はできません": "Top-up and balance check aren't available while shopping",
  お会計のあとにどうぞ: "Please do it after checkout",
  "一部お支払い済みです": "Partly paid",
  残りをお支払いください: "Please pay the rest",
  "「不足分を現金で払う」が選ばれていません": "\"Pay the rest in cash\" is not selected",
  案内を確認してから電子マネーをかざしてください: "Check the guidance, then hold your e-money card",
  "案内を確認してから、電子マネーをかざしてください": "Check the guidance, then hold your e-money card",
  "現金が投入済みです": "Cash has been inserted",
  "ポイントを使える商品がありません（宅配便・代金引換・公共料金・チケット・切手・たばこにはポイントを使えません）":
    "No items can be paid with points (not parcels, COD, bills, tickets, stamps or tobacco)",
  "クレジットカード・バーコード決済は現金と併用できません（切手・チケット・お支払いは現金のみ）":
    "Credit card and barcode pay can't be combined with cash (stamps, tickets and bills are cash only)",
  "このレジは休止中です（スタッフバーコードで再開）": "This register is closed (scan a staff barcode to reopen)",
  店舗メニューはログインしたスタッフだけが使えます: "Only logged-in staff can use the store menu",
  取引中は店舗メニューを開けません: "The store menu can't be opened during a transaction",
  在庫がありません: "Out of stock",

  // 商品
  サンドイッチ: "Sandwich",
  牛乳: "Milk",
  パン: "Bread",
  ソフトドリンク: "Soft drink",
  アルコール: "Alcohol",
  袋菓子: "Snacks",
  即席麺: "Instant noodles",
  おにぎり: "Rice ball",
  新聞: "Newspaper",
  雑誌: "Magazine",
  ライター: "Lighter",
  切手: "Stamp",
  "レジ袋 S": "Bag S",
  "レジ袋 L": "Bag L",
  フォーク: "Fork",
  スプーン: "Spoon",
  お手拭き: "Wet towel",
  "荷物を送る 発払い": "Send parcel (prepaid)",
  "荷物を送る 着払い": "Send parcel (collect)",
  荷物を受取る: "Parcel pickup",
  電気料金: "Electricity bill",
  自動車税: "Automobile tax",
  税金: "Tax",
  水道料金: "Water bill",
  ガス料金: "Gas bill",
  フライドチキン: "Fried chicken",
  スパイシーチキン: "Spicy chicken",
  アメリカンドック: "Corn dog",
  コロッケ: "Croquette",
  "焼き鳥（モモシオ）": "Yakitori (thigh, salt)",
  "焼き鳥（モモタレ）": "Yakitori (thigh, sauce)",
  大学芋: "Candied sweet potato",
  揚げ餅: "Fried rice cake",
  "ホットコーヒー S": "Hot coffee S",
  "ホットコーヒー M": "Hot coffee M",
  "ホットコーヒー L": "Hot coffee L",
  "アイスコーヒー S": "Iced coffee S",
  "アイスコーヒー M": "Iced coffee M",
  "アイスコーヒー L": "Iced coffee L",
  "ホットカフェラテ S": "Hot café latte S",
  "アイスカフェラテ S": "Iced café latte S",
  "ミルクソフト（コーン）": "Milk soft serve (cone)",
  "ミルクソフト（カップ）": "Milk soft serve (cup)",
  "チョコソフト（コーン）": "Chocolate soft serve (cone)",
  "チョコソフト（カップ）": "Chocolate soft serve (cup)",
  肉まん: "Pork bun",
  ピザまん: "Pizza bun",
  あんまん: "Red bean bun",
  カレーまん: "Curry bun",
  大根: "Daikon",
  厚揚げ: "Fried tofu",
  煮卵: "Seasoned egg",
  牛筋: "Beef tendon",
  "引換券：ソフトドリンク": "Voucher: soft drink",
  "割引券：サンドイッチ 50円引き": "Coupon: ¥50 off sandwich",
  スタッフ000: "Staff 000",
  スタッフ001: "Staff 001",
  スタッフ002: "Staff 002",
};

type Rule = [RegExp, (m: RegExpExecArray) => string];

const RULES: Rule[] = [
  [/^(\d{4})年(\d{1,2})月(\d{1,2})日 (.)曜日 (\d{2}:\d{2})$/, (m) => `${WEEKDAY[m[4]] ?? m[4]}, ${MONTH[Number(m[2]) - 1]} ${m[3]}, ${m[1]} ${m[5]}`],
  [/^(\d+)歳以上の年齢確認をしますか？$/, (m) => `Check age (${m[1]}+)?`],
  [/^(\d+)歳前後のお客様は身分証をご準備ください$/, (m) => `If you are around ${m[1]}, please have your ID ready`],
  [/^(\d+)歳以上$/, (m) => `Age ${m[1]}+`],
  [/^年確(\d+)$/, (m) => `Age ${m[1]}+`],
  [/^限定(\d+)$/, (m) => `Limit ${m[1]}`],
  [/^(.+)（追加分）$/, (m) => `${t(m[1])} (added)`],
  [/^(.+)の(お受け取り|お渡し)を確認した$/, (m) => `Confirmed ${m[2] === "お渡し" ? "hand-over" : "receipt"} of ${t(m[1])}`],
  [/^(.+)の(お受け取り数|お渡し数)$/, (m) => `${t(m[1])}: ${m[2] === "お渡し数" ? "handed" : "received"}`],
  [/^(.+)の数量$/, (m) => `Quantity of ${t(m[1])}`],
  [/^数量 (\d+)、押すとテンキーで数量を入力$/, (m) => `Quantity ${m[1]}, press to enter with the keypad`],
  [/^(.+)（押すとテンキーで入力）$/, (m) => `${t(m[1])} (press to use the keypad)`],
  [/^いまの数量 (\d+)$/, (m) => `Current ${m[1]}`],
  [/^入金済み (.+)$/, (m) => `Inserted ${yenify(m[1])}`],
  [/^お釣り (.+) をお返ししてください$/, (m) => `Return ${yenify(m[1])} in change`],
  [/^お釣り (.+) と$/, (m) => `Please take your change of ${yenify(m[1])}`],
  [/^不足額 (.+)$/, (m) => `Short by ${yenify(m[1])}`],
  [/^追加投入の受付 残り(\d+)秒$/, (m) => `You can add more for ${plural(m[1], "second")}`],
  [/^(\d+)秒の無操作で、お支払いを確定します$/, (m) => `Payment completes after ${plural(m[1], "second")} of no activity`],
  [/^あと (.+) 入れてください$/, (m) => `Insert ${yenify(m[1])} more`],
  [/^登録した (\d+)点（合計 (.+)）をすべて取り消します$/, (m) => `All ${plural(m[1], "item")} (total ${yenify(m[2])}) will be cancelled`],
  [/^入れたお金 (.+) は出金ボックスに戻ります$/, (m) => `The ${yenify(m[1])} you inserted will come back from the outlet`],
  [/^電子マネーで払った (.+) は残高に戻ります$/, (m) => `The ${yenify(m[1])} paid by e-money goes back to the balance`],
  [/^(.+)で払った (.+) は取り消します$/, (m) => `The ${yenify(m[2])} paid by ${t(m[1]).toLowerCase()} will be voided`],
  [/^(\d+)サイズ・(.+)あて$/, (m) => `Size ${m[1]}, to ${t(m[2])}`],
  [/^(\d+)サイズ・届け先未定$/, (m) => `Size ${m[1]}, destination pending`],
  [/^(\d+)サイズでお取り扱いします$/, (m) => `We'll ship it as size ${m[1]}`],
  [/^(\d+)個目(.*)$/, (m) => `Parcel ${m[1]}: ${t(m[2])}`],
  [/^取り扱いサイズを決定してください$/, () => "Decide the parcel size"],
  [/^お届け先の郵便番号を入力してください$/, () => "Enter the destination postal code"],
  [/^受付済み (\d+)個$/, (m) => `${plural(m[1], "parcel")} accepted`],
  [/^(.+)を(\d+)点回収してください$/, (m) => `Take back ${m[2]} × ${t(m[1])}`],
  [/^(.+)の数量を変更しました$/, (m) => `The quantity of ${t(m[1])} has been changed`],
  [/^(.+)を回収$/, (m) => `Take back ${t(m[1])}`],
  [/^(\d+)枚$/, (m) => plural(m[1], "slip")],
  [/^(.+) (.+)を確認した$/, (m) => `Checked ${t(m[1])} ${yenify(m[2])}`],
  [/^(.+) 不通：呼び出しが届きません$/, (m) => `${m[1]} offline: calls are not delivered`],
  [/^(\d+)秒経過$/, (m) => `${plural(m[1], "second")} elapsed `],
  [/^🔔 (.+)$/, (m) => `🔔 ${m[1].split("＆").map(t).join(" & ")}`],
  [/^担当：(.*)$/, (m) => `Staff: ${t(m[1])}`],
  [/^同じ額を (.+) で$/, (m) => `and the same amount comes out in ${yenify(m[1])} pieces from the `],
  [/^釣銭機の (.+)：残り (\d+) 枚 在高 (.+)$/, (m) => `${yenify(m[1])} in the changer: ${m[2]} left, total ${yenify(m[3])}`],
  [/^(.+)は販売期限が切れています\n(スタッフをお呼びしました|販売期限切れ商品を回収してください)(.*)$/, (m) => `${t(m[1])} is past its sell-by date\n${m[2] === "スタッフをお呼びしました" ? "We have called a staff member" : "Collect the expired item"}`],
  [/^(.+)の販売期限が切れています$/, (m) => `${t(m[1])} is past its sell-by date`],
  [/^(.+)：スタッフをお呼びしました(.*)$/, (m) => `${t(m[1])}: we have called a staff member`],
  [/^(.+)が選ばれています（(.+)は使えません）$/, (m) => `${t(m[1])} is selected (${t(m[2])} can't be used)`],
  [/^あと (.+)円 入れてください$/, (m) => `Insert ¥${m[1]} more`],
  [/^(.+)cm を超える荷物は受け付けられません$/, (m) => `Parcels over ${m[1]} cm can't be accepted`],
  [/^(.+)kg を超える荷物は受け付けられません$/, (m) => `Parcels over ${m[1]} kg can't be accepted`],
  [/^(.+)は1回のお会計で(\d+)点までです$/, (m) => `${t(m[1])}: up to ${m[2]} per purchase`],
  [/^(.+)は欠品です（FF ボタンの長押しで予約注文として登録できます）$/, (m) => `${t(m[1])} is sold out (long-press to pre-order)`],
  [/^電子マネーの残高が足りません（残高 (.+)円・お支払い (.+)円）$/, (m) => `Not enough e-money (balance ¥${m[1]}, due ¥${m[2]})`],
  [/^内税 (.*)$/, (m) => `Tax incl. ${yenify(m[1])}`],
];

function t(s: string): string {
  return trCore(s) ?? yenify(s);
}

export function trCore(raw: string): string | null {
  if (raw.includes("\n")) {
    const lines = raw.split("\n").map((l) => trCore(l));
    return lines.every((l) => l !== null) ? lines.join("\n") : null;
  }
  const s = raw.replace(/\s+/g, " ").trim();
  if (!JP.test(s)) return yenify(s);
  if (Object.hasOwn(DICT, s)) return DICT[s];
  for (const [re, f] of RULES) {
    const m = re.exec(s);
    if (!m) continue;
    const r = f(m);
    if (r) return r;
  }
  const y = yenify(s);
  if (!JP.test(y)) return y;
  return null;
}

export const misses = new Set<string>();

export function trText(s: string): string {
  if (!JP.test(s)) return s;
  if (s.includes("\n")) {
    const r = trCore(s);
    if (r !== null) return r;
  }
  const m = /^(\s*)([\s\S]*?)(\s*)$/.exec(s)!;
  const r = trCore(m[2]);
  if (r === null) {
    misses.add(m[2]);
    return s;
  }
  return `${m[1]}${r}${m[3]}`;
}

type HtmlRule = [RegExp, (m: RegExpExecArray) => string];

const HTML_RULES: HtmlRule[] = [
  [
    /^合計 <span class="count"><b>(\d+)<\/b>点(<small class="full">上限<\/small>)?<\/span> <b class="total">([\d,]+)<\/b><span class="yen">円<\/span>$/,
    (m) =>
      `Total <span class="count"><b>${m[1]}</b> ${Number(m[1]) === 1 ? "item" : "items"}${m[2] ? `<small class="full">Max</small>` : ""}</span> <span class="yen">¥</span><b class="total">${m[3]}</b>`,
  ],
  [/^チケット(<span class="bill-n">)(\d+)枚(<\/span>)を発券しました$/, (m) => `${m[1]}${plural(m[2], "ticket")}${m[3]} issued`],
  [/^(.+)は現金のみのお取り扱いです$/, (m) => `${m[1].split("・").map((x) => t(x)).join(", ")} can only be paid in cash`],
  [/^チケット(<span class="bill-n">)(\d+)枚(<\/span>)$/, (m) => `${m[1]}${plural(m[2], "ticket")}${m[3]}`],
  [/^チケット (\d+)枚をお渡ししました$/, (m) => `Handed over ${plural(m[1], "ticket")}`],
  [/^公共料金(<span class="bill-n">)(\d+)枚(<\/span>)を受け付けます$/, (m) => `Accepting ${m[1]}${plural(m[2], "utility bill")}${m[3]}`],
  [/^公共料金(<span class="bill-n">)(\d+)枚(<\/span>)　支払いが完了しました$/, (m) => `${m[1]}${plural(m[2], "utility bill")}${m[3]} paid`],
  [/^公共料金(<span class="bill-n">)(\d+)枚(<\/span>)$/, (m) => `Utility bills ${m[1]}${plural(m[2], "slip")}${m[3]}`],
  [/^受付 (<span class="bill-n">)(\d+)枚(<\/span>)<span class="bill-sum">合計 (.+)<\/span>$/, (m) => `Accepted ${m[1]}${plural(m[2], "slip")}${m[3]}<span class="bill-sum">Total ${yenify(m[4])}</span>`],
  [
    /^電子マネーの残高 <b>(.+?)<\/b> を引き出し、残り <b>(.+?)<\/b> を現金でお支払いいただきます$/,
    (m) => `We'll draw the e-money balance of <b>${yenify(m[1])}</b> and you pay the remaining <b>${yenify(m[2])}</b> in cash`,
  ],
  [/^不足額 <b>(.+?)<\/b>$/, (m) => `Short by <b>${yenify(m[1])}</b>`],
  [/^電子マネーの残高<span class="emoney-balance">(.+?)<\/span>$/, (m) => `E-money balance<span class="emoney-balance">${yenify(m[1])}</span>`],
  [
    /^(.+?)で (.+?) お支払い済み、残り <b>(.+?)<\/b> のお支払い方法をお選びください$/,
    (m) => `${yenify(m[2])} paid by ${t(m[1]).toLowerCase()}. Please choose how to pay the remaining <b>${yenify(m[3])}</b>`,
  ],
  [/^(.+?)で (.+?) お支払い済み、残り <b>(.+?)<\/b> を現金で$/, (m) => `${yenify(m[2])} paid by ${t(m[1]).toLowerCase()}. Please pay the remaining <b>${yenify(m[3])}</b> in cash`],
  [/^現金 <b>(.+?)<\/b> を出金ボックスからお返しください$/, (m) => `Return <b>${yenify(m[1])}</b> in cash from the outlet`],
  [/^現金 <b>(.+?)<\/b> をお返しします$/, (m) => `We'll return <b>${yenify(m[1])}</b> in cash`],
  [/^切手・チケット・お支払いの <b>(.+?)<\/b> は現金のみ（併用できるのは電子マネー）$/, (m) => `Stamps, tickets and bills (<b>${yenify(m[1])}</b>) are cash only (only e-money can be combined)`],
  [
    /^(.+?)で <b>(.+?)<\/b>、切手・チケット・お支払いの <b>(.+?)<\/b> はそのあと現金で$/,
    (m) => `<b>${yenify(m[2])}</b> by ${t(m[1]).toLowerCase()}, then stamps, tickets and bills (<b>${yenify(m[3])}</b>) in cash`,
  ],
  [/^あと <b>(.+?)<\/b> 不足しています$/, (m) => `<b>${yenify(m[1])}</b> short`],
  [
    /^(\d+)サイズ・(.*?)あて（〒(.*?)）<b class="parcel-fee">(.+?)<\/b>$/,
    (m) => `Size ${m[1]}, to ${t(m[2])} (〒${m[3]})<b class="parcel-fee">${t(m[4])}</b>`,
  ],
  [/^長さ(<button[\s\S]*<\/button>)<span>cm<\/span>$/, (m) => `Length${m[1]}<span>cm</span>`],
  [/^重さ(<button[\s\S]*<\/button>)<span>kg<\/span>$/, (m) => `Weight${m[1]}<span>kg</span>`],
  [/^(<span class="parcel-no">)(\d+)個目(<\/span>)(.+)$/, (m) => `${m[1]}Parcel ${m[2]}${m[3]}${t(m[4])}`],
  [/^入金する金額を下の<b>「入金」ボックス<\/b>に入れて Enter を押してください$/, () => "Put the amount to deposit into the <b>inlet</b> below and press Enter"],
  [
    /^両替する金額を<b>「入金」ボックス<\/b>に入れて Enter を押してください<br>同じ額を (.+?) で<b>「出金」ボックス<\/b>に出します$/,
    (m) => `Put the amount to exchange into the <b>inlet</b> and press Enter<br>The same amount comes out of the <b>outlet</b> in ${yenify(m[1])} pieces`,
  ],
];

const SKIP = "[data-i18n-skip]";
const ATTRS = ["title", "aria-label", "placeholder"];

function localizeEn(html: string): string {
  const tpl = document.createElement("template");
  tpl.innerHTML = html;
  const root = tpl.content;
  for (const el of root.querySelectorAll<HTMLElement>("*")) {
    if (!root.contains(el) || el.closest(SKIP)) continue;
    for (const a of ATTRS) {
      const v = el.getAttribute(a);
      if (v) el.setAttribute(a, trText(v));
    }
    if (!JP.test(el.innerHTML)) continue;
    const inner = el.innerHTML.replace(/\s+/g, " ").trim();
    for (const [re, f] of HTML_RULES) {
      const m = re.exec(inner);
      if (!m) continue;
      el.innerHTML = f(m);
      el.dataset.i18nDone = "";
      break;
    }
  }
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  for (let n = walker.nextNode(); n; n = walker.nextNode()) nodes.push(n as Text);
  for (const n of nodes) {
    const p = n.parentElement;
    if (p?.closest(SKIP) || p?.closest("[data-i18n-done]")) continue;
    const next = trText(n.data);
    if (next !== n.data) n.data = next;
  }
  root.querySelectorAll("[data-i18n-done]").forEach((el) => el.removeAttribute("data-i18n-done"));
  return tpl.innerHTML;
}

const memo = new Map<string, string>();

export function localize(html: string, lang: Lang): string {
  if (lang === "ja" || !html) return html;
  const hit = memo.get(html);
  if (hit !== undefined) return hit;
  const out = localizeEn(html);
  if (memo.size > 400) memo.clear();
  memo.set(html, out);
  return out;
}
