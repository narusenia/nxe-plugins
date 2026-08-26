# 引き継ぎ

**2026-08-26 時点。** Doubler が `doubler-v0.1.0` として一区切りついたところ。

このファイルは**そのとき何が動いていて、次に触る人が最初に知るべきこと**を
1 枚にまとめたもの。設計の正は各仕様書、状態の正は
[`implementation/backlog.md`](implementation/backlog.md) にある。

## 5 分で状況を掴む順番

1. [`../AGENTS.md`](../AGENTS.md) — リポジトリの地図と規約
2. [`README.md`](README.md) — どの文書が何を担当するか
3. [`implementation/backlog.md`](implementation/backlog.md) の「現在地」 — 今どこか
4. [`../.agents/rules/vizia.md`](../.agents/rules/vizia.md) — **UI を触るなら先に読む**

## 何ができているか

| | |
|---|---|
| `plugins/doubler` | NXE Doubler。CLAP + VST3。`doubler-v0.1.0` |
| `crates/nxe-ui` | 共通ウィジェット・テーマ・Lucide アイコン。`mise run gallery` で単体起動 |
| `crates/nxe-dsp` | 共通の解析（`Handoff` / `PanScope` / `Spectrum`） |
| CI | `check`（PR と main への push）、`release`（`<plugin>-v<version>` タグ） |

テスト 103 本。CPU はエンジン 70 µs + 解析 15 µs で、予算 533 µs の 16%。

## 触る前に知っておくと事故らないこと

このリビジョンの vizia には**コンパイルも通り、エラーも出さず、何もしない**
ものがいくつかある。全部 [`../.agents/rules/vizia.md`](../.agents/rules/vizia.md)
に理由付きで書いてあるが、特に刺さりやすいのは:

- **CSS の `font-size` は単位を受け付けない。** `12px` はパース失敗で宣言ごと
  捨てられ、既定の 16 px になる。この 1 つに気づくまで、文字サイズを何度
  変えても何も変わらなかった
- **`cx.add_timer` は baseview で一度も発火しない。** `process_timers` を呼ぶのは
  `vizia_winit` だけ。周期処理は `cx.spawn` の `ContextProxy`
- **未指定のサイズは `Auto` ではなく `Stretch(1.0)`。** 中身に合わせたい階層は
  全部に `Auto` と書く
- **当たり判定を持つ子は親の押下を飲む。** ツールチップの中身も含む

作業のしかたの罠は [`../.agents/rules/rust.md`](../.agents/rules/rust.md) の
「Things that cost time here」に。**「直したつもりで直っていない」を疑う前に、
まず二分探索する** — この開発で一番時間を溶かしたのはそこ。

## 残っていること

| 何 | どこ | なぜ残っているか |
|---|---|---|
| `DBL-13` 既定値の詰め | `plugins/doubler/docs/implementation/doubler-plan.md` | **実素材と耳が要る。** ボーカル・ギター・シンセで `dsp.md` の「耳で詰める定数」を確認する |
| 値の直接入力 | `crates/nxe-ui/src/entry.rs` | widget はあり gallery では動くが、プラグインに載せると editor の表示が更新されなくなる。**原因未特定**。潰した仮説はそのファイルの冒頭に |
| `UI-8` / `UI-9` | `implementation/nxe-ui-plan.md` | Doubler が使わない。**使う相手が現れるまで作らない** |
| 混ざったコミット `f89b40c` | — | フォント修正と Dry Gain 削除が同じコミットに入っている。分けるなら push 済み履歴の書き換えが要る |

## 次にプラグインを足すとき

`docs/specifications/architecture.md` の「新しいプラグインを足す」に手順がある。
`nxe-ui` と `nxe-dsp` はそのまま使える。`crates/nxe-ui/README.md` が
「この道具立てで UI を組む」ガイド。

**共通クレートに何を上げるかの線引き**（`architecture.md`）:

- **解析は最初から共通**（どのプラグインでも同じものだから）
- **音を作る部品は 2 個目が必要とするまで `<plugin>-core` に置く**（1 個目の
  時点では何が共通か分からず、先に作ると「1 つしか実装が無い抽象」が残る）

## リリースのしかた

```bash
git tag -a doubler-v0.1.1 -m 'NXE Doubler 0.1.1'
git push origin doubler-v0.1.1
```

タグからプラグイン名とバージョンを読み、3 OS でビルドして**下書き** Release に
zip を付ける。**バージョンはプラグインの `Cargo.toml` が持つ**ので、タグと
食い違うと CI が落ちる（わざとそうしてある）。macOS は未署名なので、公開前に
README の Gatekeeper 手順が生きていることを確かめる。
