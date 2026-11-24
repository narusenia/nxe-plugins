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

### V2-10 / V2-11 / V2-12 — Velour 2 / Air 2 / Diorama 2

V2-9 の型に載せる。完了条件は V2-7 と同じ。

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
| **Mix 系の書体。** Inter + letter-spacing で進める。Host Grotesk や allo に替えるなら、バイナリへの埋め込みが許されるライセンスかを先に確かめる | V2-9 で型を確定するとき |
| **ステータスバー（ポインタの説明）を残すか、どこに置くか。** H4 の案には描いていない | V2-9 |
| **Colour / Design シリーズの型。** 候補は Colour が H1 系、Design が G 系 | 各シリーズに着手するとき |
| v0 から持ち越した耳の単位（`DBL-13` の既定値、`DIO-13` の既定値、Sparkleur の主観サインオフ `SPK-18`、Diorama の Advanced の偏差 `REQ-DIO-009`） | 各プラグインを型に載せる単位の中で。DSP は `<plugin>-core` にあるので、移行とは独立に進められる |
