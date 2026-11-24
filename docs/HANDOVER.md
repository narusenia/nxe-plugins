# 引き継ぎ

**2026-11-24 時点。** v0.2.0 は 5 本とも出荷済み（2026-08-31）。いまは **v2 の
実装中**。Doubler が truce に載った（`V2-1` のコード、#7）。v0.x の引き継ぎ（測定の経緯、各プラグインで踏んだ罠、
vizia の重さ問題）は [`archive/v0-handover.md`](archive/v0-handover.md)。

## いま作っているもの — v2

フレームワークを nih-plug + vizia から **truce + egui** に移し、5 本を
**`NXE <name> 2`（`com.nxe.<name>2`）として `2.0.0` から出し直す**。既存
セッションとの互換は捨て、旧版と並べて入れられるよう ID も名前も新しくする。

- 決定と単位: [`implementation/v2-plan.md`](implementation/v2-plan.md)
- 状態: [`implementation/backlog.md`](implementation/backlog.md)
- 順序の根拠: [`implementation/roadmap.md`](implementation/roadmap.md)
- 見た目の型: [`specifications/design-system.md`](specifications/design-system.md)

## 次の一手

**`V2-1` の実機確認**（Ableton Live・Studio One・Logic で保存→再読込、自動化）
は人の手が要る。`cargo truce install --user --clap --vst3 --au2 -p doubler`
で入る。

コードの側は `V2-2`〜`V2-4` と `V2-13` が着手できる（`backlog.md`）。

- `release/v0` は切ってある。v0.2 系のバグ修正はそちら
- `au_manufacturer` と 5 本の `fourcc` は `truce.toml` で決めた。**出荷後は変えられない**

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
