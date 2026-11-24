# v2 計画 — truce + egui へ移し、5 本を「2」として出し直す

> 計画日: 2026-11-24。状態は [`backlog.md`](backlog.md)、順序の根拠は
> [`roadmap.md`](roadmap.md)。見た目の型は
> [`../specifications/design-system.md`](../specifications/design-system.md)。

v0.2.0 までの 5 本（Doubler / Velour / Sparkleur / Air / Diorama）を、
**フレームワークごと載せ替えて**、窓を作り直し、`NXE <name> 2` として
`2.0.0` から出し直す。v0.x の経緯は [`../archive/`](../archive/)。

## 決めたこと

| # | 決定 | 理由 |
|---|---|---|
| 1 | UI を vizia から egui に移す。**旧デザインの再現はせず、新デザインで直接書く** | vizia の制約（`letter-spacing`・`tnum`・`line-height` が無い、CSS とパレットの二重管理、反転面で地が黒落ちする類の罠）と、vizia / baseview の fork 保守をやめるため。デザインはどのみち刷新するので、再現は捨て仕事になる |
| 2 | フレームワークを nih-plug から [truce](https://github.com/truce-audio/truce)（6.3.0）に移す | AUv2 / AUv3 / AAX をネイティブに出せる。`cargo truce screenshot --check` で窓の回帰を CI で検出でき、`cargo truce run` で DAW 無しに窓を開ける。`cargo truce validate` が auval / pluginval / clap-validator をまとめて回す。nice-plug（0.4、experimental）も比べたが、AU が無い |
| 3 | **既存セッションとの互換は捨てる** | truce の VST3 TUID は `FNV-1a-128(plugin id)` 固定で、出荷済みの `*b"NXEDoubler......"` を再現できない。パラメータ ID も u32 で、nih-plug の文字列 ID のハッシュと一致しないので自動化が外れる。truce を fork して合わせる道は、vizia fork と同じ保守を背負うので取らない |
| 4 | **名前も ID も新しくする**: `NXE Doubler 2`、`bundle_id = "doubler2"` → `com.nxe.doubler2`。5 本とも（Diorama も出荷済みなので同じ） | ID を据え置くと、旧セッションが新版で開いて**状態が読めず既定値で鳴る**。黙って音が変わるのは、別プラグインとして見えるより悪い。新 ID なら旧版と並べて入れられ、旧セッションは旧版で開く |
| 5 | 窓のワードマークは `Doubler` のまま。「2」は版の表示として扱う | `ui.md` の「一覧で読んだものを窓で繰り返さない」 |
| 6 | 版は 5 本とも `2.0.0` から。**5 本そろえて同時に出す**。その前に Doubler 2 だけ beta を自分用に出し、制作で数週間使う | 「5 本で 1 製品」を守りつつ、ホスト固有の問題を beta で出し切る。beta の時点で `bundle_id` は確定値にする |
| 7 | 出すもの: **macOS は CLAP + VST3 + AUv2、Windows は CLAP + VST3**。配布は今と同じく GitHub Actions の zip、署名なし | Windows でも使っているので外さない。署名と notarize は `INFRA-3` の宿題のまま、2.0.0 の条件にしない。AUv3 / AAX は需要が見えてから |
| 8 | `cargo truce validate` とスクリーンショット差分を、CI で macOS と Windows の両方で回す | UI の欠陥はテストに映らない（`ui.md`）を、画像の差分で映るようにする |
| 9 | **描画の予算は 1 コアの 10 %**（v0.2.0 の gallery 計測と同じ条件）。超えたら表現を削る | vizia fork で 22.6 % → 約 10 % まで落とした水準を下回らない。egui は immediate mode で毎フレーム窓全体を組むので、ここが移行の一番のリスク |
| 10 | 信号の表示を滑らかにする: 描画フレームごとに dt 基準の指数補間（表示側だけ、DSP は触らない）、曲線は単調三次補間でつなぐ、光は補間後の値にだけ乗せる | 今のカクつきの原因は、30 Hz のハートビートで読んだ値を 60 Hz で描いていること（2 フレームに 1 回しか動かない）と、少数バンドの折れ線。単調三次はオーバーシュートしないので、無いピークを描かない |
| 11 | 見た目の規則のうち、**解くもの**: ウェイト 2 段（Regular / Medium）、letter-spacing、**動いている信号にだけ**光（グロー・ブルーム）と曲線。**残すもの**: 1 窓 1 アクセントと 5 色の家族、グリッドと罫線、共通の窓幅、影なし、角丸ゼロ、記号は直線 | 「構造はスイス、信号だけが光る」。詳細は `design-system.md` |
| 12 | シリーズで見た目を分ける。共有するのはグリッド・書体・罫線・窓幅・部品で、シリーズごとに変えるのは**色の家族と光・動きの量（強度）** | 今後の Colour シリーズ（カラーベース向け）と Design シリーズ（ベースの音作り）を、同じブランドのまま性格だけ変えて受け止める |
| 13 | 既存 5 本（Mix 系）の型は **H4 の左サイドバー版**（左のサイドバーと上の主図を L 字の反転面でつなぐ） | デザイン案の比較で選んだ。反転面が 1 つにまとまるので、強さを出しても `ui.md` の「反転面 2 つまで」に余裕が残る |
| 14 | 移行の順番: **Doubler（技術リスク）→ `nxe-ui` の egui 版をギャラリーでまとめて作る → Sparkleur（型を確定）→ 残り 3 本** | 技術リスク → 型 → 横展開。根拠は `roadmap.md` |
| 15 | 作業は main から切ったブランチで行い、PR でまとめる。docs は整理しながら進める（v0.x の文書は `docs/archive/` へ） | — |
| 16 | **Pumice は「2」ではなく `NXE Pumice` の `1.0.0` として出す**（`bundle_id = "pumice"` → `com.nxe.pumice`、`fourcc = "Pumi"`）。nih-plug のまま出さず、型が決まってから truce + egui に載せる | 一度も公開していない（タグも release も無い）ので、互換を捨てる理由も「2」を付ける理由も無い。nih-plug で出すと、出した直後に載せ替えになる |
| 17 | **全プラグインにプリセットを付ける。**「持たない」の決定（`REQ-DBL-013`・`REQ-VEL-020`・`REQ-SPK-021`・`REQ-AIR-022`・`REQ-DIO-021`・`REQ-GLU-021`）を取り消す。中身は truce の機構（付属の `.preset`、ホストへの公開、`PresetStore`）、窓の中は `nxe-ui` の共通部品 1 つ | 使ってみて、無いと不便だった。「マクロが足りていればプリセットは要らない」は作り手の理屈で、使う側は作った音を保存して呼び戻したい。既定値を耳で詰める仕事はそのまま残る（既定値は最初に開いたときの顔） |
| 18 | **窓は移植ではなく作り直す。** 各プラグインで、操作したことが主図にすぐ見えて、何が起きているかが絵で分かることを目標にする | 決定 1 の「再現しない」だけでは、同じ情報を新しい部品で並べ直すだけになりうる。刷新するなら、分かりやすさを上げる機会にする |

### 判断の途中で捨てた案

- **nih-plug の rev を上げて `nih_plug_egui` を使う** — 一度はこれに決めた。truce が
  AU とスクリーンショット検証を持っていて、互換を捨てられると分かった時点で置き換えた
- **nice-plug** — nih-plug の直系で互換は保てそうだったが、AU が無く、0.4 で
  experimental。互換を捨てると決めたので、互換の利点が消えた
- **clap-wrapper で AUv2 を包む** — truce がネイティブに出すので不要になった

## 単位

状態は [`backlog.md`](backlog.md) が正。ここは内容と完了条件だけを持つ。
**文書の書き換えは、その前提を変えたコードと同じ単位で行う**（実装が正、という
`AGENTS.md` の原則のため）。

### V2-1 — truce の骨格と Doubler 2 の起動

ワークスペースに truce を入れ、Doubler 2 を truce の `PluginLogic` で鳴らす。
UI は egui の空の窓でよい。`doubler-core` は触らない。

- `truce.toml` の `[vendor]`（`id = "com.nxe"`、`au_manufacturer`）と
  Doubler 2 の `[[plugin]]`（`bundle_id = "doubler2"`、`fourcc`）を決める。
  **`au_manufacturer` と `fourcc` は出荷後に変えられない**ので、ここで 5 本分まとめて決める
- パラメータ宣言を truce の `#[derive(Params)]` / `#[param(...)]` に書き直す
- vizia 版の 5 本は、この単位の**前**に `release/v0` ブランチを main から切って残す
  （v0.2 系のバグ修正はそちらで行う）
- 書き換える文書: `AGENTS.md`（フレームワーク、AU の範囲、ID 規則）、
  `LICENSING.md`（**この単位の時点では混在**: Doubler 2 は `vst3-sys` が外れて
  MIT OR Apache-2.0 になるが、まだ nih-plug に乗っている Velour / Sparkleur / Air /
  Diorama のラッパと `nxe-plug-ui` は GPL-3.0-only のまま。どれが残るかと理由を
  書く。truce 自身のライセンスの但し書きが配布物に影響しないことも確かめて書く）、
  `docs/specifications/architecture.md`

**決めたこと**: `au_manufacturer = "Nxeu"`、`fourcc` は `Dbl2` / `Vlr2` /
`Spk2` / `Air2` / `Dio2`、`bundle_id` は `<name>2`。`truce.toml` が正。
パラメータ ID は truce の既定（フィールド名のハッシュ）にした。並べ替えに強く、
フィールド名を変えたときだけ壊れる。

**完了条件**: macOS で CLAP / VST3 / AUv2 が `cargo truce validate` を通る。
Ableton Live・Studio One・Logic で読み込め、保存 → 再読込で全パラメータが戻り、
自動化を書いて再生すると追従する。

移行中は nih-plug 版と truce 版が同じワークスペースに並ぶ。**全 crate が
MIT OR Apache-2.0 になったと `LICENSING.md` に書くのは、最後の `nih_plug` 依存を
外す単位**（V2-10〜V2-12 のうち最後に終わったもの）で行う。

### V2-2 — マウス合成の修正を引き継ぐ

v0.x で一番効いた修正は、baseview が `[NSEvent setMouseCoalescingEnabled: NO]`
を**ホストのプロセス全体**に掛けて戻さなかったこと（v0 の
[`ui-frame-cost.md`](../investigations/ui-frame-cost.md)）。truce の窓の層に同じ
問題があるか確かめ、あれば直す。直し方は上流への PR を先にし、待てない分だけ
patch で持つ。

**完了条件**: Doubler 2 の窓を開閉したあとも、ホストの UI がポインタに遅れずに
付いてくる（実機）。

**コードを読んだ結果（2026-11-24）: 該当なし。** truce 6.3.0 の窓の層
（`baseview-truce` 0.1.1-truce.13、`truce-egui`、`truce-clap` / `truce-vst3` /
`truce-au`）のどこにも `setMouseCoalescingEnabled` の呼び出しが無い。
`baseview-truce` の `updateTrackingAreas` はトラッキング領域を張り直すだけで、
v0 の baseview が足していた 1 行を持っていない。patch も上流への PR も要らない。
残っているのは実機の確認だけ。

### V2-3 — 描画予算を測る

決定 9 の予算を、Doubler 2 の窓で測る。測り方は v0 と同じ（`sample`、
v0.2.0 の gallery と同じ条件）。

- 入力も信号の変化も無いフレームは再描画を頼まない
- グローは毎フレームぼかしを計算せず、事前に描いたものを使い回す

**完了条件**: 数字を `docs/investigations/` に記録し、予算内に収まっている。
収まらないなら、何を削ったかも記録する。

### V2-4 — 窓の回帰を CI で検出する

`cargo truce screenshot --check` を CI（macOS / Windows）に入れる。基準画像は
リポジトリに置く。

**完了条件**: 窓をわざと 1 px ずらした変更で CI が落ちることを 1 回確かめる。

### V2-5 — `nxe-ui` の egui 版とギャラリー

共通部品を egui で作り直し、全部品を 1 つのギャラリーで開けるようにする
（v0 の `examples/gallery.rs` の役割）。

- トークン: パレットの 5 色家族（OKLCH の明度・彩度をそろえる）とテスト
  `the_palettes_are_one_family`、窓幅の 1 定数、高さは部品の合計、を引き継ぐ
- 部品: ノブ、バー、セグメント、読み値（固定幅・右寄せ・符号常時）、主図の面、
  曲線、メーター、ステータス、**サイドバー（H4 の左版）**
- `nxe-plug-ui` は truce のパラメータと `nxe-ui` をつなぐ唯一の crate のまま残す。
  `nxe-ui` は truce を知らない
- v0 で差し戻した「値の直接入力」（`UI-3`）を、egui の上でもう一度載せる
- **プリセット欄**（決定 17）: 名前の表示、前後送り、一覧からの読込、ユーザー
  プリセットの保存。値の読み書きは各プラグインのアダプタが truce の
  `PresetStore` に渡し、`nxe-ui` は truce を知らないまま
- 書き換える文書: `.agents/rules/ui.md`（決定 11 の解禁分）、
  `.agents/rules/vizia.md` は `docs/archive/` へ、`crates/nxe-ui/README.md`

**完了条件**: 全部品がギャラリーにあり、引き継いだテストが通る。

**決めたこと**（実装で）:

- クレートは `crates/nxe-egui`。vizia の `nxe-ui` は 4 本（Pumice 含め 5 本）
  がまだ使うので横に並べ、最後の vizia プラグインが移る `V2-16` で消す。
  **パレットはそれまで 2 か所にあり、変えるなら両方**
- 部品はパレットを**引数で受け取る**。vizia は木をさかのぼって探したが、egui
  には木が無い。反転面は子に `Palette::inverted` を渡すだけで、v0 の
  「反転はパレットであって別の概念ではない」はそのまま成り立つ
- 値の部品は値を書かず、`Gestures`（begin / change / end / reset / edit）と
  打ち込まれた文字列を返す。truce の `begin_edit` / `set_param` /
  `end_edit` に渡すのはプラグイン側のアダプタ
- フォントは Inter Regular / **Medium**（v4.1、Regular と同じ版）と Geist
  Mono。`nxe-ui/assets/` から読み、ファイルは 1 つずつのまま
- ギャラリーは eframe（dev-dependency のみ）。プラグインには入らない
- `vizia.md` は vizia のプラグインが残るあいだ要るので、`docs/archive/` へ
  移すのは `V2-16`。`ui.md` には v2 の解禁分を書いた

**実装で分かったこと**:

- **egui は新しいフォントを次のパスから効かせる。** 登録前の名前付き
  ファミリー（Medium）を使うと panic する。`nxe_egui::window` は 1 パス目に
  登録だけして `false` を返し、呼び手はそのフレームを地だけで終える
- **そのため truce の `screenshot!`（1 パスしか描かない）では、自前の
  フォントが写らない。** truce-egui の `with_font` は 1 書体しか取れない。
  `V2-7` で窓を載せる前に、truce-egui に `FontDefinitions` を渡す口を足す
  （patch → 上流）か、スクリーンショットだけ 2 パス描かせる必要がある
- `ui.scope_builder` は中身が使った幅を親に割り当てるので、横並びの中で
  使うと親のカーソルが**戻る**（サイドバーの右に主図が重なった）。固定の
  領域に子を置くときは `ui.new_child`

### V2-6 — 信号表示の平滑化

決定 10 を `nxe-ui` の部品に入れる。

**完了条件**: 30 Hz で値を送る試験信号を 60 Hz で描いても、メーターと曲線が
階段状に動かない（ギャラリーで目視、スクリーンショット列で確認）。

**決めたこと**（実装で）: `nxe_egui::smooth`。時定数は `TAU = 40 ms`（30 Hz の
読みの間を埋め、過渡はまだ過渡に見える長さ）。値が着いたら再描画の要求を
止める。完了条件の「スクリーンショット列」は、**毎フレーム動くことを確かめる
単体テスト**（`a_30_hz_reading_drawn_at_60_hz_moves_every_frame`）に置き換えた。
画面を撮る間隔をフレームに合わせる手段が無く、テストのほうが同じことを
毎回確かめられる。ギャラリーには 30 Hz の読みと `Smoothed` / `Raw` の切り替えを
置いたので、目視の比較はそこでできる。

**描く速さは 30 Hz に下げた**（`V2-7` で測って、60 Hz は予算の 10 % に入らな
かった。`docs/investigations/v2-frame-cost.md`）。決定 10 の「60 Hz で描いて
も階段にならない」は単体テストで残し、実際の窓は 30 Hz で描く。

### V2-7 — Doubler 2 を型に載せる

V2-1 の仮の窓を、V2-5 の部品と H4 の型で置き換える。

**完了条件**: 全パラメータに操作子がある（v0 から引き継ぐ `params` 走査テスト）。
**主なパラメータを動かすと主図が目に見えて変わる**（決定 18。動かしても絵が
変わらない操作子は、図か説明のどちらかが足りていない）。**プリセット欄があり、
付属プリセットが入っている**（決定 17。中身を耳で決めるところは「耳が要る」）。
解析の受け渡しが全部書かれている（`every_handoff_is_published` を引き継ぐ）。
スクリーンショットの基準を登録した。実機で見た。**決定 9 の予算を、型に載った
窓でもう一度測って収まっている**（V2-3 は空の窓なので、それだけでは証明に
ならない）。条件は V2-3 と同じ。

**決めたこと**（実装で）:

- 窓は 880 × 438。サイドバーの主題は**相関**（ワイドナーが利用者に負っている
  1 つの数字、`nxe_dsp::Correlation`）。マクロは 6 つ（Detune / Delay /
  Spread / Humanize / Mix / Output）をサイドバーに 3 段
- 主図は Voice Field（v0 の図を作り直したもの）。決定 18 に合わせて、**主な
  マクロが全部図を動かす**: Spread は扇の開き、Delay は点の大きさ、Detune は点を
  通る棒の幅、Humanize は点の揺れ、Mix は ▲ の半径。点のドラッグで Pan と
  Gain、対の点は鏡像で追随する
- 下の列は `Tone` / `Voices` のタブ。Tone は棚の曲線を wet のスペクトラムの上に
  描き、曲線上のハンドルで Tone Lo / Hi を動かす。Voices は 8 声 × 4 軸の表
  （バーは形状、数値は実効値）
- **ミラーのスイッチは載せていない。** 点のドラッグは Pan と Gain を常に鏡像で
  動かし（v0 の既定と同じ）、表のバーは 1 声ずつ動く。軸ごとの切り替えが要るかは
  beta で使って決める
- **付属プリセットは Rust の表**（`ui/presets.rs`）で、マクロだけを書く。声の
  形状はダブラーの性格なので触らない。truce の `presets/` ディレクトリ（ホストの
  ブラウザに出る形）にはまだしていない — 入れ子の `delay` / `detune` /
  `pan` / `gain` が 8 声と重なってフィールド名で書けず、数値 ID になるため。
  **中身は仮で、耳で決める**
- ユーザープリセットは truce の `PresetStore` のユーザー領域に全パラメータを
  保存する（全フォーマット共通の場所なので、ホストからも見える）

### V2-8 — Doubler 2 beta

Doubler 2 だけを beta として出し（タグは `doubler-v2.0.0-beta.N`）、制作で数週間使う。
タグは `release.yml` の `*-v*` に当たるので、**beta は V2-13 で truce に置き換えた
リリースの CI で作る**（今の `cargo xtask bundle` の経路では作れない）。配布は
draft release の zip。

**完了条件**: 使って見つかった問題が、単位か積み残しとして backlog に載っている。

### V2-9 — Sparkleur 2（型を確定する）

部品が一番多い窓で、H4 の型を確定させる。ここで決まった寸法・余白・書体の段は
`design-system.md` に書き戻す。

**完了条件**: V2-7 と同じ。加えて、型の寸法が `design-system.md` にある。

**決めたこと**（実装で）:

- truce への移植（`NXE Sparkleur 2` / `sparkleur2` / `Spk2`、MIT OR
  Apache-2.0）。**全パラメータに `parse` を付けた** — 一部にだけ付けると
  clap-validator の `param-conversions` が、読み返せないもの（Solo の `Off`）で
  落ちる
- 窓は 880 × 430（サイドバーの 3 段で決まる）。サイドバーの主題は**一番大きく
  動かされている帯の量**（符号付き dB）。マクロは 9 つを 3 × 3（Spark /
  Character / Snap、Body / Air / Focus、Speed / Mix / Output）
- 主図は Band Field。帯ごとに**届く範囲**（静かな信号と大きな信号に対して帯が
  する量、`transfer_db` から）を薄い面で、**今の動き**を濃い棒で描く。決定 18
  の「動かすと絵が変わる」は届く範囲が受け持つ: Spark・Character・Body・Air・
  Lift・Mode・帯の重みとトリムが全部これを動かし、Focus は帯の境界を動かす。
  帯をドラッグするとその帯の Gain（トリム）。Sparkle のゲートは Presence と
  Air の上の線の明るさ、De-Harsh は Presence に数字で出す
- 下の列は `Bands` / `Dynamics` のタブ。Bands は 5 帯 × Up / Down / Gain /
  Solo の表。Dynamics は Lift / Punch / De-Harsh / Sub Protect のつまみ、
  Sparkle と De-Harsh の読み値、選んだ帯の入出力曲線（今の位置の点つき）
- **Mix と Output と Speed は主図を動かさない。** Mix と Output は帯の動きでは
  なく、Speed は時間の量なので静止した図に出ない。つまみの値で読む
- 付属プリセットは 6 つ（Default / Polish / Gloss / Vocal Air / Drum Snap /
  Crush）でマクロだけを書く。**中身は仮**で、耳で決める
- 型の寸法は `design-system.md` に書き戻した。ステータスバーは置かない

移植で変わったことがもう 1 つある。**Solo の 5 つは自動化できるようになった。**
truce の `flags` には「自動化しない」だけを指定する方法が無く、`readonly` は
窓からの変更も止めてしまう。保存された Solo が付いたまま開くと壊れて聞こえる
ので、窓で Solo の状態がはっきり見えるようにする。

### V2-10 / V2-11 / V2-12 — Velour 2 / Air 2 / Diorama 2

V2-9 の型に載せる。完了条件は V2-7 と同じ。

**V2-10 で決めたこと**（実装で）:

- truce への移植（`NXE Velour 2` / `velour2` / `Vlr2`、MIT OR Apache-2.0）。
  全パラメータに `parse`（`nxe_egui::text`）。Solo は Sparkleur と同じ理由で
  自動化できるようになった。両極のつまみ（帯ごとの Texture と Bias）の表示は
  v0 の `0.00` から符号付きの % にした（Sparkleur と同じ）
- 窓は 880 × 430。サイドバーの主題は**レイヤー**（足している音の大きさを、
  入力に対して dB で。2 つのスペクトラムのエネルギー比）。並列のサチュレーター
  は「どれだけ足しているか」で読むもので、メーターは dry が支配するので
  それを言えない。マクロは 3 × 3（Drive / Texture / Density、Body / Presence /
  Air、Focus / Mix / Output）
- 主図は Band Field。dry を塗り、足している音を線で重ねる。帯ごとに、フェーダー
  の高さの柱（ガードが引いている分は上から削れる）と、**その帯の曲線**
  （`curve_for`）を小さな四角に描く。決定 18 は曲線が受け持つ: Drive・
  Texture・帯の Texture・Bias・Mode が曲線を変え、Focus は帯の範囲を動かす。
  帯のドラッグはその帯のフェーダー
- 下の列は `Bands`（3 帯 × Texture / Bias / Solo）と `Advanced`（Harsh
  Guard / Sib Guard / Emotion と、ガードが今引いている量）。**Density・Mix・
  Output・Emotion は主図を動かさない**（Density と Emotion は信号に対して
  働くので静止した図に出ない）
- 付属プリセット 6 つ（Default / Warm / Clear Vocal / Breath / Edge / Push）。
  マクロだけを書く。Default は `VEL-17` で耳で決めた既定値で、残りは仮

**V2-11 で決めたこと**（実装で）:

- truce への移植（`NXE Air 2` / `air2` / `Air2`、MIT OR Apache-2.0）。mono in /
  stereo out は残した（`REQ-AIR-011`）。インスタンスごとのノイズの種は
  `AirState` に持ち、`reset` で作り直しても変えない。v0 の「パラメータ ID が
  出荷したものと同じ」テストは外した — truce の ID はフィールド名のハッシュで、
  v2 は v0 のセッションと互換を持たない（決定 4）
- 窓は 880 × 430。主題は Velour と同じ**レイヤー**（足している音の大きさを
  入力に対して dB で）。マクロは Surface / Blend / Character、Focus / Width /
  Follow、Mix / Output
- 主図は入力の塗りと足した層の線に、**層の置き場所**を重ねる: `corner_of`
  から上の帯で、Focus が左端、Surface が高さ、Blend が倍音（縦の線）と
  ノイズ（粒）の割合、Character が線と粒の詰まり方、Width が上の棒の幅。
  左上の 3 本は検出器で、枠が Follow の深さ、塗りが今の動き。ドラッグで
  Surface
- 下の列は `Follow`（3 つの偏差と今の動き）と `Curve`（Drive / Bias / Guard
  と、ガードが引いている量、層の相関）。v0 の「タブ無し 1 画面」
  （`REQ-AIR-013`）は H4 の型に合わせて崩した
- 付属プリセット 6 つ（Default / Silk / Breath / Shimmer / Wide Air /
  Static）。マクロだけを書く。Default 以外の中身は仮

**V2-12 で決めたこと**（実装で）:

- truce への移植（`NXE Diorama 2` / `diorama2` / `Dio2`、MIT OR
  Apache-2.0）。mono in / stereo out は残した（`REQ-DIO-011`）。v0 の
  「パラメータ ID が出荷したものと同じ」テストは外した（Air と同じ理由）。
  パレットの名前は `PARALLAX` のまま（`nxe-ui` と同時に替えるもの）
- 窓は 880 × 430、**タブ無し**（`REQ-DIO-013` の 1 画面を残した）。マクロ
  8 つをサイドバーに置き、下の列は測った値（Direct / Reflected / Clarity /
  Room Corr.）だけ。主題は **D / R**（直接音と反射の比、dB）
- 主図は到着の並び（時間 = 距離の 1 本の軸）。**窓が自分のエンジンを持って
  マクロから解く**（`DIO-17` の「止まっていても設定できる」を、ホストが
  `process` を呼ばない standalone でも守るため）。Mix と Output 以外の
  マクロが全部図を動かす。ドラッグで Depth
- 付属プリセット 6 つ（Default / Close / Behind / Far Back / Small Room /
  Wide Room）。Output 以外のマクロを書く。Default 以外の中身は仮

### V2-15 — Pumice を main に入れる

`pumice` ブランチ（nih-plug、`PUM-11` の耳の手前まで）を main に入れる。
窓は vizia のまま。DSP（`pumice-core`）はこの時点で本線に乗り、v2 の移行と
独立に耳の単位（`PUM-11`）を進められる。

**完了条件**: main で `mise run check` が通る。

### V2-16 — Pumice 1.0（truce に載せる）

決定 16。V2-9 の型に載せ、`NXE Pumice` / `com.nxe.pumice` / `Pumi` で出す。
**レイテンシを申告するのは 6 本で Pumice だけ**なので、truce の `latency` で
PDC が通ることを v0 と同じ反転試験（`PUM-1`）で確かめ直す。`REQ-PUM-014`
（AU は対象外）は truce で AU が出せるので取り消す。

**完了条件**: V2-7 と同じ。加えて、Ableton（VST3）と Studio One（CLAP）、
Logic（AU）で反転して無音。

**決めたこと**（実装で）:

- truce への移植（`NXE Pumice` / `pumice` / `Pumi`、1.0.0、MIT OR
  Apache-2.0）。2→2 と 1→1 は残した（`REQ-PUM-011`）。エンジンは 2 ch を
  まとめてその場で処理するので、ブロックはホストの最大ブロック長の作業領域を
  通す（`reset` で確保）。レイテンシは `PluginLogic::latency` が返し、truce が
  ブロックごとに比べて変わった時だけホストに伝える
- 周波数と幅は**単位のまま**宣言した（`log(20, 20000)` Hz と
  `log(0.1, 4)` oct）。nih-plug では `0..=1` の位置を手で写していた。
  出荷していないので保存形式の互換は要らない。図の横軸は周波数パラメータの
  正規化値そのもの
- Quality は自動化できるようになった（Solo と同じ理由）。自動化で変われば
  レイテンシも変わり、truce がそれを伝える
- 窓は 880 × 430、**タブ無し**（`REQ-PUM-013`）。主題は**削減量**（いちばん
  深い所、dB）。マクロはサイドバーに Depth / Sharpness / Speed、Threshold /
  Mix / Output、Low / High。上の帯に Result / Delta と Low / Normal / High
- 主図はスペクトラム（床から塗る）、削減（天井から下げる）、重み（線、
  `nodes::weight_at` から）。範囲の外は影で、両端の線をドラッグで動かす。
  ノードは図の上で置く: ドラッグで周波数と深さ、両端の印で幅、空き所の
  ダブルクリックで追加、ノードのダブルクリックで削除。ポインタの下の
  ノードの値は横に出す（H4 にステータスバーが無いので）
- 図の下は**ノードを 1 つずつ**: 番号（1〜6、オンのものに印）、On、
  Freq / Width / Depth のバー。6 列並べるとバーが 25 px になる。図で
  触ったノードがここに来る
- スペクトラムは入力のピークがある時だけ描く。最初のブロックの前は
  ハンドオフが 0（= 0 dB）で、全面が塗られていた
- 付属プリセット 6 つ（Default / Gentle / Firm / Harsh Top / Boxy Mids /
  Parallel）。主のつまみと範囲だけを書き、**ノードは書かない**（ノードは
  その声のどこが鳴るかで、プリセットが動かすと一般にできない部分を壊す）。
  Default 以外の中身は仮

### V2-13 — リリースの CI

**V2-8（beta）の前に行う。** `release.yml` を truce のビルドに置き換える。
nih-plug のまま残っている 4 本のタグも壊さないよう、移行中は両方の経路を持つ。タグは今と同じ `<plugin>-v<version>`、
1 タグ 1 プラグイン。

**完了条件**: macOS と Windows の zip が draft release に付く。`cargo truce validate`
が両 OS で通らなければ落ちる。

**決めたこと**: truce 版は Linux を出さない（決定 7 のとおり）。nih-plug に
残っている間のタグは今までどおり 3 OS。macOS は `cargo truce package` で
universal（Apple Silicon + Intel）を作り、installer 用に stage された bundle を
zip にする（installer 自体は配らない）。`cargo truce build` はホストの
アーキテクチャしか作らず、`--target` を付けると 6.3.0 は AU の dylib を
見失う。
`release.yml` を触る PR では、Doubler を release せずにビルドと validate だけ回す。

### V2-14 — 5 本の 2.0.0 と Pumice 1.0.0

5 本を実機で並べて見て、聴いて、`2.0.0` を出す。Pumice は同じ型の窓なので
同じ回に `1.0.0` で出す。README の表を差し替える。

## まだ決めていないこと

| 何 | いつ決めるか |
|---|---|
| **Colour / Design シリーズの型。** 候補は Colour が H1 系、Design が G 系 | 各シリーズに着手するとき |
| v0 から持ち越した耳の単位（`DBL-13` の既定値、`DIO-13` の既定値、Sparkleur の主観サインオフ `SPK-18`、Diorama の Advanced の偏差 `REQ-DIO-009`） | 各プラグインを型に載せる単位の中で。DSP は `<plugin>-core` にあるので、移行とは独立に進められる |
