# 引き継ぎ

**2026-11-24 時点。** v0.2.0 は 5 本とも出荷済み（2026-08-31）。いまは **v2 の
実装中**。Doubler が truce に載り（#7）、窓のスクリーンショット差分（#10）と
truce のリリース経路（#11）が CI に入った。Pumice は main に入った（#9）。
egui の部品（`crates/nxe-egui`、#14）と信号の平滑化（#15）ができ、**Doubler 2 が
H4 の窓とプリセットに載った**（#16）。v0.x の引き継ぎ（測定の経緯、各プラグインで踏んだ罠、
vizia の重さ問題）は [`archive/v0-handover.md`](archive/v0-handover.md)。
Pumice の v0 期の経緯（`ADAPTIVE` を落とした理由、PDC のゲート）は
[`archive/pumice-handover.md`](archive/pumice-handover.md)。

## いま作っているもの — v2

フレームワークを nih-plug + vizia から **truce + egui** に移し、5 本を
**`NXE <name> 2`（`com.nxe.<name>2`）として `2.0.0` から出し直す**。既存
セッションとの互換は捨て、旧版と並べて入れられるよう ID も名前も新しくする。

- 決定と単位: [`implementation/v2-plan.md`](implementation/v2-plan.md)
- 状態: [`implementation/backlog.md`](implementation/backlog.md)
- 順序の根拠: [`implementation/roadmap.md`](implementation/roadmap.md)
- 見た目の型: [`specifications/design-system.md`](specifications/design-system.md)

## 次の一手

**人の手が要るもの**（`cargo truce install --user --clap --vst3 --au2 -p doubler`
で入る。Logic・Cubase・Ableton・Bitwig が手元にある。窓だけなら
`cargo truce run -p doubler` で DAW 無しに開く）:

- `V2-1`: 保存→再読込で全パラメータが戻るか、自動化に追従するか
- `V2-2`: 窓を開閉したあとホストの UI がポインタに付いてくるか。**コード側に
  原因は無い**（`v2-plan.md` の V2-2）
- `V2-3`: 描画予算。**音を通したホストで**測る。standalone では窓の分が
  30 Hz で約 10 %、境目にいる。60 Hz は約 18 % で入らなかった
  （[`investigations/v2-frame-cost.md`](investigations/v2-frame-cost.md)）
- `V2-7`: 窓を実機で見る（点のドラッグ、Tone のハンドル、値の直接入力、
  プリセットの保存と読込）。**付属プリセット 5 つの中身は仮**で、耳で決める

**コードの側の次は `V2-9`**（Sparkleur 2。型の寸法を確定させる単位）。
`V2-7` の残りは全部人の手なので、待たずに進められる。Doubler 2 の窓の作り
（`plugins/doubler/doubler/src/ui/`）が手本で、**truce との結線は `ui/bind.rs`、
プリセットは `ui/presets.rs`** の形をそのまま使う。

**v2 の途中で足した決定**（`v2-plan.md` 決定 16〜18）: Pumice は `NXE Pumice`
1.0.0、全プラグインにプリセット、窓は作り直して「操作が主図に見える」ように。

**上流（truce）に送るもの**は backlog の「まだ単位にしていないもの」に 3 つ
（CLAP の state 読込後の rescan、AU v2 の名前、truce-egui の書体の口）。
`patches/` に truce-clap と truce-egui の 2 つを当てている。cargo-truce の Windows の
clap-validator 引数のずれ（`release.yml` に回避あり）も一緒に。**出す前に確認**。

- `release/v0` は切ってある。v0.2 系のバグ修正はそちら
- `au_manufacturer` と 6 本の `fourcc` は `truce.toml` で決めた。**出荷後は変えられない**
- **手元の `auval` は信用しない。** DAW には出るのに `didn't find the component`
  になる（他社の AU も同時に消える）。CI の auval が正
- **egui は新しい書体を次のパスから効かせる。** 窓の中で登録すると 1 パス目は
  panic するか既定の書体になる。プラグインは `EguiEditor::with_font_definitions`
  （`patches/truce-egui`）で渡す
- **truce-egui は遅延付きの再描画要求を「今すぐ」と扱っていた**（patch 済み）。
  窓が毎ティック描いて CPU を食う。再描画の頻度を変えたら standalone で
  フレーム数を数えて確かめる
- **スクリーンショットの基準画像は CI で焼く。** 手元の GPU とは縁の画素が
  1 段ずれる。`pixel_threshold(2)` で吸収していて、1 px ずれは今も落ちる
- ビルドは kache（`~/.cargo/config.toml` の `rustc-wrapper`）。kache が消えると
  cargo が全部止まる。mise で入れている（グローバル）

## 5 分で状況を掴む順番

1. [`../AGENTS.md`](../AGENTS.md) — リポジトリの地図と規約。移行中の
   2 フレームワークの並び方もここ
2. [`implementation/v2-plan.md`](implementation/v2-plan.md) の「決めたこと」
3. [`implementation/backlog.md`](implementation/backlog.md) の「今すぐ着手できるもの」
4. UI を触るなら [`specifications/design-system.md`](specifications/design-system.md)
   と [`../.agents/rules/ui.md`](../.agents/rules/ui.md)。`ui.md` の v2 向けの
   書き換えは `V2-5` で、それまでは `design-system.md` の解禁分が先に進んでいる

## v2 でも生きている罠

v0 で払った代償のうち、フレームワークを替えても残るもの。

- **ホストに窓のリサイズを頼まない。** 窓を広げる開閉で Ableton が固まった
- **寸法は実機でしか決まらない。** 最初に書いた数字は外れると思っておく。
  見た目がおかしいときは**スクリーンショットの画素を測る**
- **UI の欠陥はテストに映らない。** v2 ではスクリーンショット差分（`V2-4`）で
  映るようにするが、それまでは実機で見る
- **解析の受け渡しは「書かれていない」ほうが気づかれない。** 窓は全部ゼロで、
  信号の無いトラックと見分けがつかない。`every_handoff_is_published` を引き継ぐ
- **タグは 1 回の push に 1 つ。** 4 つまとめて push したとき、GitHub は
  ワークフローを 1 つも作らず、エラーもどこにも出なかった
- **音声スレッドで確保しない。** `<plugin>-core` の約束で、フレームワークを
  替えても変わらない

## まだ決めていないこと

[`implementation/v2-plan.md`](implementation/v2-plan.md) の「まだ決めていない
こと」が正。大きいものは、Linux を 2.0.0 に入れるか、Mix 系の書体、ステータス
バーの置き場所、Colour / Design シリーズの型。
