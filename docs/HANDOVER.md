# 引き継ぎ

**2026-11-25 時点。** v0.2.0 は 5 本とも出荷済み（2026-08-31）。**v2 のコードは
終わった。** 6 本とも truce + egui に載り（Doubler #16、Sparkleur、Velour #20、
Air #21、Diorama #22、Pumice #26）、nih-plug と vizia の側は消した（#28）。
リリースはシリーズ単位になった（#27、#29）。残りは人の手が要るもの（実機で見る、
耳で既定値とプリセットを決める）と、リリースそのもの。v0.x の引き継ぎは
[`archive/v0-handover.md`](archive/v0-handover.md)、Pumice の v0 期の経緯は
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

**人の手が要るもの**（`mise run install <plugin>` で CLAP / VST3 / AU が入る。
**先にホストを閉じる**。Logic・Cubase・Ableton・Bitwig が手元にある。窓だけなら
`cargo truce run -p <plugin>` で DAW 無しに開く）:

- `V2-1`: 保存→再読込で全パラメータが戻るか、自動化に追従するか
- `V2-2`: 窓を開閉したあとホストの UI がポインタに付いてくるか。**コード側に
  原因は無い**（`v2-plan.md` の V2-2）
- `V2-3`: 描画予算。**音を通したホストで**測る
  （[`investigations/v2-frame-cost.md`](investigations/v2-frame-cost.md)）
- `V2-7` / `V2-9` / `V2-10`〜`V2-12` / `V2-16`: 6 本の窓を実機で見る。
  **付属プリセットの中身は全部仮**で、耳で決める
- `V2-16`: Pumice の PDC を Ableton（VST3）・Studio One（CLAP）・Logic（AU）の
  反転で確かめる。Quality を変えるとレイテンシが変わり、CLAP では再起動を
  頼む（`patches/truce-clap`）。ホストが本当に再起動するかも見る

**その後のリリース**（`V2-8` の beta → `V2-14`）。タグは
`basic-<YYYY.MM>[-beta.N]` 1 つで、`series.toml` の Basic 6 本がまとめて 1 つの
draft release に乗る（プラグインごとの zip と、全部入りの zip）。**draft を
公開するのは人**。README のプラグイン表の差し替えは `V2-14` で。

**v2 の後**は [`implementation/series-plan.md`](implementation/series-plan.md)
（Colour / Design / Create / Kit / Peek）。窓の型は `design-system.md` の
「シリーズと強度」で決めてある。

**v2 の途中で足した決定**（`v2-plan.md` 決定 16〜18）: Pumice は `NXE Pumice`
1.0.0、全プラグインにプリセット、窓は作り直して「操作が主図に見える」ように。

**上流（truce）に送るもの**は backlog の「まだ単位にしていないもの」に 3 つ
（CLAP の state 読込後の rescan、AU v2 の名前、truce-egui の書体の口）と、
CLAP の `latency.changed` を `activate` の中だけで呼ぶ修正。
`patches/` に truce-clap、truce-egui、baseview-truce の 3 つを当てている（baseview は
描画タイマーを common modes に入れる 1 行。default mode だけだとリサイズ中に描画が止まる）。cargo-truce の Windows の
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

1. [`../AGENTS.md`](../AGENTS.md) — リポジトリの地図と規約
2. [`implementation/v2-plan.md`](implementation/v2-plan.md) の「決めたこと」
3. [`implementation/backlog.md`](implementation/backlog.md) の「今すぐ着手できるもの」
4. UI を触るなら [`specifications/design-system.md`](specifications/design-system.md)
   と [`../.agents/rules/ui.md`](../.agents/rules/ui.md)

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
  ワークフローを 1 つも作らず、エラーもどこにも出なかった（シリーズ単位の
  リリースでタグは 1 回 1 つになったが、規則は残す）
- **CI の全部の検査を見てからマージする。** 1 つだけ落ちていたのを見落とした
  ことがある（#27 の macOS。#29 で直した）
- **音声スレッドで確保しない。** `<plugin>-core` の約束で、フレームワークを
  替えても変わらない

## まだ決めていないこと

[`implementation/v2-plan.md`](implementation/v2-plan.md) の「まだ決めていない
こと」が正。Linux は 2.0.0 に入れない（nih-plug の経路と一緒に消した）。
