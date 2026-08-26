# 引き継ぎ

**2026-08-26 時点。** Doubler は `doubler-v0.1.0` を**公開済み**、Velour は
`velour-v0.1.0` で**一区切り**（下書き Release）。**Sparkleur は `SPK-3` まで —
`nxe-audio`、クロスオーバーのゲート、帯域ごとの検波が入っている。まだ音は出ない**
（ラッパは `SPK-8`）。

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
| `plugins/doubler` | NXE Doubler。CLAP + VST3。`doubler-v0.1.0` **公開済み** |
| `plugins/velour` | NXE Velour。CLAP + VST3。パラメータ 22 個。UI・解析・既定値まで完了。`velour-v0.1.0` |
| `crates/nxe-audio` | 共通の**処理**（`shaper` / `oversample` / `biquad` / `envelope` / `guard` / `harmonics`）。`SPK-1` で `velour-core` から抜いた |
| `crates/nxe-ui` | 共通ウィジェット・テーマ・アイコン。`mise run gallery` |
| `crates/nxe-dsp` | 共通の解析（`Handoff` / `PanScope` / `Spectrum` / `Level`） |
| `plugins/sparkleur/sparkleur-core` | **`SPK-3` まで。** 5 帯域クロスオーバー（LR4 の木 + オールパス補正、`FOCUS`）と検波（帯域ごとのパワー、`SPEED` と帯域中心からの床）。ゲイン計算はまだ |
| `plugins/sparkleur/docs` | 要件・DSP 仕様・UI 仕様・実装計画（`SPK-1`〜`SPK-18`）。`sparkleur` ラッパクレートは無い |
| CI | `check`（PR と main への push）、`release`（`<plugin>-v<version>` タグ） |

テスト 265 本。CPU は予算 533 µs に対し **Doubler 85 µs / Velour 128 µs**
（`VEL-16`。Velour の内訳はエンジン 4x が 79、`Spectrum` 48 バンド × 2 が 45、
`Level` × 4 が 4）。

## 次にやること

**`SPK-4`（ゲイン計算、上下コンプ）。製品の核。** 下げ・上げ・上限・床・ニー。
`Detector::decibels()` が読み値を返し、**その目盛りは帯域の RMS より
0.05〜2.5 dB 上**（下の罠）。`SPK-3` から送られた宿題が 1 つある —
**`SPEED` 最速・50 Hz で THD が上がらないこと**。ゲインが付いて初めて測れる。

**ここまで通っている**: `SPK-1` の `nxe-audio`（`shaper` / `oversample` /
`biquad` / `envelope` / `guard` / `harmonics`、`guard` は `RelativeGuard<N>`）、
`SPK-2` のクロスオーバー（44.1〜192 kHz・`FOCUS` 全域で和が ±0.1 dB 以内）、
`SPK-3` の検波（帯域ごとのパワー、`SPEED` を帯域中心の周期で下から抑える床）。

**この 2 つは Sparkleur のコードを 1 行も書かずに着手できる**:

| | 何 | なぜ先か |
|---|---|---|
| `SPK-10` | `nxe_ui::band::Band` の `reduction` → 符号付き `delta` | 上げコンプが半分の製品で、絵が上げを描けない |
| `SPK-11` | `param_bind` を新クレートに共通化 | Doubler と Velour で同内容が 2 つ。**3 個目が要求した** |

**設計で解いた一番大きい問題は v1 の線引き。** 概念文書は 4 製品ぶん
（OTT / Exciter / Widener / Transient）あって、`roadmap.md` 自身が「v1 の
線引き自体が難問」と書いていた。WIDTH と PUNCH を v2 に置いた理由は
`REQ-SPK-019` / `REQ-SPK-020` にある。

## 仮のまま残してある定数

**動かす理由が出てから触る。** 既定値は `VEL-17` で決めた（`DRIVE` 80 /
`BODY` 40 / `PRESENCE` 60 / `AIR` 40 / `TEXTURE` 50 / `DENSITY` 50 / `MIX` 80）。

| 何 | 聴きどころ |
|---|---|
| `TEXTURE` のトリム 9 個 | Warm → Edge で **−3.0 dB** レベルが動く残差を吸収するためにある。一番「仮」なところ |
| Guard のしきい値 −8 / −14 dB | 普通のボーカルで**動かない**のが正。動くなら厳しい |
| `EMOTION` の係数 3 つ | 0.50 / 0.40 / 0.30。声量差で質感が変わるのが**分かるが不安定でない**か |
| `REFERENCE_DB` −18 dB | `EMOTION` の軸の中心と `DENSITY` の支点を兼ねる 1 個の数。動かすと両方に効く |
| `BIAS_LEVEL_DB` 0 | 知覚的な密度をどれだけ返すか。機構は仕様、量は耳 |

## Velour で踏んだ罠（同じ形を 3 回やった）

**テスト信号の周波数を「周期数」で書くな。** 周期数はバッファ長とサンプルレートに
依存するので、間違えても落ちず「符号が逆の測定値」として出てくる。`VEL-2` /
`VEL-8` / `VEL-9` で 3 回踏んだ。**`harmonics::tone(amplitude, hz, rate, length)`
と `bin_of(hz, rate, length)` を足したので、以後は周波数で書く。**

**「往復すれば分かる」は嘘のことがある。** オーバーサンプラの位相の割り当てを
逆にしていたが、往復テストは通り低域の正弦も無事に戻ってきた。壊れていたのは
**その間に居る非線形が見る信号**で、イメージが −88 dB ではなく −24 dB だった。
`the_upsampler_leaves_no_image` がそれを固定している。

**エンベロープが絡むものはブロックごとに `set_shape` して測れ。** テストヘルパ
`rendered` は最初のサンプルの前に 1 回だけ呼ぶので、検波器はまだ無音しか
見ていない。`EMOTION` の向きが**全部逆に測れた**。ホストと同じ形で回す
`blocked` を別に用意してある。

**テストが「何も検出できないまま通る」ことがある。** `VEL-10` のジッパーの
テストは、可動域の 25% を 1 段で動かしても通っていた（エンジンにパラメータで
跳ねるゲインが 1 つも無いので、実は正しい）。**測定が失敗できることを同じテスト
の中で確かめる**行を足してある。

**引き算で「足した層」を取り出すな。** `出力 − 原音` は原音の方が大きいと層の
精度をほとんど捨てる。`SOLO` を全部オンにして層を直接取る。

## Sparkleur で踏んだ罠

**ここまで落ちたテストは 4 本とも「実装ではなく期待値」が間違っていた。**
落ちたら先に実測を並べて理論値と突き合わせるほうが速い。

**分割の裾は 24 dB/oct とは限らない**（`SPK-2`）。木構造なので各帯域は自分の
境界だけでなく**上流の全部のハイパス**を通っている。band 5 の 6 kHz より
1 オクターブ下は `HP(1500)` がまだ効いていて 30 dB/oct で落ちる。24 dB/oct を
測るなら境界が 1 つしか効かないところ。**帯域の幾何中心も 0 dB ではない**
（band 2 は 1.7 オクターブ幅で中心が −1.5 dB）。

**検波値は帯域の RMS ではない**（`SPK-3`）。非対称な 1 次追従は平均に落ち着かず、
**RMS の +0.05〜+2.54 dB** に座る。位置を決めるのは attack/release の比だけ
（比 4 で +0.05〜0.9、比 20 で +2.5）。素直なコンプの挙動だが、**しきい値を
計算で出せない**ということでもある — `SPK-18` を耳でやる根拠。

**定常正弦は時定数の床を測る道具として弱い**（`SPK-3`）。50 Hz では release の
20 ms が単独で読み値を持ち上げるので、attack が 1 ms でも 33 ms でもリプルは
1.0 dB のまま変わらない。**「対照実験が差を出さない」形の失敗**で、`VEL-10` の
「測定が失敗できることを同じテストで確かめる」行がそれを捕まえた。差が出るのは
**ステップ応答**（床のある帯域と無い帯域で 33 倍）。

**仕様の数字は 4 回動いた。** `k` の上限 20 → 6 → 8、`bias` のレベル補正
6 dB → 0、`DENSITY` のメイクアップの基準 full scale → `REFERENCE_DB`。
どれも実測が仕様を否定した結果で、**理由と実測値を `dsp.md` と
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
| `DBL-13` 既定値の詰め | `doubler-plan.md` | **耳が要る** |
| Velour の `SOLO` が保存される | `params.rs` | nih-plug に逃げ道が無い。**画面には出るようにした** — Advanced の `ON` と、`BandField` が他の区画を落とすこと（`band.rs` の `soloing`）。図はタブに関係なく常に見えるので、ラッチしたまま開いても分かる |
| 値の直接入力 | `crates/nxe-ui/src/entry.rs` | gallery では動くがプラグインに載せると editor の表示が止まる。**原因未特定** |
| `UI-9` `ToggleSwitch` | `nxe-ui-plan.md` | **2 個のプラグインがどちらも要らなかった。** 3 個目でも要らなければ落とす |
| 混ざったコミット `f89b40c` / `68d5199` | — | それぞれフォント修正 + Dry Gain 削除、コード + 文書の一部。分けるなら push 済み履歴の書き換えが要る |

## 次にプラグインを足すとき

`docs/specifications/architecture.md` の「新しいプラグインを足す」に手順がある。
`nxe-ui` と `nxe-dsp` はそのまま使える。

**3 個目は Sparkleur**（マルチバンドダイナミクス + Harmonic Sparkle）。
順序の根拠は [`implementation/roadmap.md`](implementation/roadmap.md) の
「Velour の後」。材料は `SPK-1` で `nxe-audio` に移してある。

**「上げるのは 2 個目が要求してから」は当たったが、境界は 2 か所ずれた**
（`SPK-1`）。**測定ヘルパの `harmonics` も一緒に動く** — 移すモジュールの
テストが全部それで書かれていて、`nxe-audio` は `velour-core` に依存できない。
**耳で決めた数は残す** — `envelope` は機構だけ移し、attack 5 ms /
release 150 ms と `REFERENCE_DB` は `velour-core` の `envelope::vocal()` に
置いた。4 個目を足すときも同じ 2 つを見る。

## リリースのしかた

```bash
git tag -a doubler-v0.1.1 -m 'NXE Doubler 0.1.1'
git push origin doubler-v0.1.1
```

タグからプラグイン名とバージョンを読み、3 OS でビルドして**下書き** Release に
zip を付ける。**バージョンはプラグインの `Cargo.toml` が持つ**ので、タグと
食い違うと CI が落ちる（わざとそうしてある）。
