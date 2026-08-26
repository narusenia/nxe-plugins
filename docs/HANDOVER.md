# 引き継ぎ

**2026-08-26 時点。** Doubler が `doubler-v0.1.0` で一区切り、Velour が
`VEL-9` / `VEL-8` まで進んで**音が出る状態**。

このファイルは**そのとき何が動いていて、次に触る人が最初に知るべきこと**を
1 枚にまとめたもの。設計の正は各仕様書、状態の正は
[`implementation/backlog.md`](implementation/backlog.md) にある。

## 5 分で状況を掴む順番

1. [`../AGENTS.md`](../AGENTS.md) — リポジトリの地図と規約
2. [`implementation/backlog.md`](implementation/backlog.md) の「現在地」 — 今どこか
3. 触るプラグインの `plugins/<name>/docs/implementation/<name>-plan.md` の
   **該当単位の「決めたこと」と「踏んだ罠」** — ここに時間を溶かした記録がある
4. [`../.agents/rules/vizia.md`](../.agents/rules/vizia.md) — **UI を触るなら先に読む**

## 何ができているか

| | |
|---|---|
| `plugins/doubler` | NXE Doubler。CLAP + VST3。`doubler-v0.1.0`（**下書き Release。未公開**） |
| `plugins/velour` | NXE Velour。CLAP + VST3。**UI 無し**、パラメータ 20 個。Live で音を確認済み |
| `crates/nxe-ui` | 共通ウィジェット・テーマ・アイコン。`mise run gallery` |
| `crates/nxe-dsp` | 共通の解析（`Handoff` / `PanScope` / `Spectrum` / `Level`） |
| CI | `check`（PR と main への push）、`release`（`<plugin>-v<version>` タグ） |

テスト 202 本。Doubler の CPU はエンジン 70 µs + 解析 15 µs / 予算 533 µs。
**Velour の CPU は未測定**（`VEL-16`）。

## 次にやること

**`VEL-6`（エンベロープ検波と EMOTION）→ `VEL-7`（DENSITY）。**
`velour-plan.md` に完了条件がある。`VEL-6` が作る検波器を `VEL-7` が共有する
（`EMOTION` と `DENSITY` は**同じエンベロープの別の使い方**で、検波は
**圧縮前の入力**を見る — ここが 2 つの衝突を解く唯一の点）。

その後は `VEL-10`（詰め）→ `VEL-11`〜`VEL-15`（UI）→ `VEL-16`（予算）→
`VEL-17`（耳）。

## 耳での確認を待っているもの

コードは動いていて、**良し悪しの判断だけが残っている**もの。

| 何 | 聴きどころ |
|---|---|
| `VEL-4` の `TEXTURE` | Warm と Edge が**別の質感**に聞こえるか。Warm → Edge で **−3.0 dB** レベルが動くのが気になるか |
| `VEL-8` の Guard | 普通のボーカルで**動かない**か（動くならしきい値が厳しい）。入力ゲインを ±12 dB しても効き方が変わらないか |
| `VEL-9` の `SOLO` | 各帯域が何を足しているか。`Air Bias` +1 の質感 |
| 既定値すべて | **全部仮。** プリセットを持たない方針（`REQ-VEL-020`）なので既定値が製品の顔。`VEL-17` でまとめて詰める |

## Velour で踏んだ罠（同じ形を 3 回やった）

**テスト信号の周波数を「周期数」で書くな。** 周期数はバッファ長とサンプルレートに
依存するので、間違えても落ちず「符号が逆の測定値」として出てくる。`VEL-2` /
`VEL-8` / `VEL-9` で 3 回踏んだ。**`harmonics::tone(amplitude, hz, rate, length)`
と `bin_of(hz, rate, length)` を足したので、以後は周波数で書く。**

**「往復すれば分かる」は嘘のことがある。** オーバーサンプラの位相の割り当てを
逆にしていたが、往復テストは通り低域の正弦も無事に戻ってきた。壊れていたのは
**その間に居る非線形が見る信号**で、イメージが −88 dB ではなく −24 dB だった。
`the_upsampler_leaves_no_image` がそれを固定している。

**引き算で「足した層」を取り出すな。** `出力 − 原音` は原音の方が大きいと層の
精度をほとんど捨てる。`SOLO` を全部オンにして層を直接取る。

**仕様の数字は 3 回動いた。** `k` の上限 20 → 6 → 8、`bias` のレベル補正
6 dB → 0。どれも実測が仕様を否定した結果で、**理由と実測値を `dsp.md` と
`velour-plan.md` に残してある**。同じ数字を触るなら先にそこを読む。

## Doubler で踏んだ罠

このリビジョンの vizia には**コンパイルも通り、エラーも出さず、何もしない**
ものがある。全部 [`../.agents/rules/vizia.md`](../.agents/rules/vizia.md) に
理由付きで書いてあるが、特に刺さりやすいのは:

- **CSS の `font-size` は単位を受け付けない。** `12px` はパース失敗で宣言ごと
  捨てられ、既定の 16 px になる
- **`cx.add_timer` は baseview で一度も発火しない。** 周期処理は `cx.spawn`
- **未指定のサイズは `Auto` ではなく `Stretch(1.0)`**
- **当たり判定を持つ子は親の押下を飲む**

**「直したつもりで直っていない」を疑う前に、まず二分探索する** —
この開発で一番時間を溶かしたのはそこ。

## 残っている宿題

| 何 | どこ | なぜ残っているか |
|---|---|---|
| `doubler-v0.1.0` の**公開** | GitHub Releases | 下書きのまま。公開はユーザーの判断。未署名なので README の Gatekeeper 手順を先に確認する |
| `DBL-13` 既定値の詰め | `doubler-plan.md` | **耳が要る** |
| Velour の `SOLO` が保存される | `params.rs` | nih-plug に逃げ道が無い。ラッチしたまま保存されると壊れて聞こえるので、**`VEL-14` で画面上に状態がはっきり出ること**が要件 |
| 値の直接入力 | `crates/nxe-ui/src/entry.rs` | gallery では動くがプラグインに載せると editor の表示が止まる。**原因未特定** |
| `UI-9` `ToggleSwitch` | `nxe-ui-plan.md` | **2 個のプラグインがどちらも要らなかった。** 3 個目でも要らなければ落とす |
| 混ざったコミット `f89b40c` / `68d5199` | — | それぞれフォント修正 + Dry Gain 削除、コード + 文書の一部。分けるなら push 済み履歴の書き換えが要る |

## 次にプラグインを足すとき

`docs/specifications/architecture.md` の「新しいプラグインを足す」に手順がある。
`nxe-ui` と `nxe-dsp` はそのまま使える。

**3 個目は Sparkleur**（マルチバンドダイナミクス + Harmonic Sparkle）。
順序の根拠は [`implementation/roadmap.md`](implementation/roadmap.md) の
「Velour の後」。**Velour の `velour-core` にある移動候補**
（`shaper` / `oversample` / `biquad` / `guard`）が Sparkleur の材料で、どれも
Velour を知らないように書いてあるので、移動はファイルの移動だけで済む。
**共通クレートに上げるのは Sparkleur が実際に要求してから**
（`architecture.md`）。

## リリースのしかた

```bash
git tag -a doubler-v0.1.1 -m 'NXE Doubler 0.1.1'
git push origin doubler-v0.1.1
```

タグからプラグイン名とバージョンを読み、3 OS でビルドして**下書き** Release に
zip を付ける。**バージョンはプラグインの `Cargo.toml` が持つ**ので、タグと
食い違うと CI が落ちる（わざとそうしてある）。
