# 実装バックログ

全計画の実装単位を 1 枚に並べたもの。**着手できるものを探すためのファイル**で、
設計の正は各計画書にある。

- 単位の内容・完了条件は計画書を見る。ここには要約しか書かない
- 計画書を更新したらこの表も更新する。片方だけ直さない
- 完了した単位は行を消さず `✅` にして、PR 番号かコミット SHA を入れる
  （main に直接コミットしている間は SHA）
- **順序の判断は [`roadmap.md`](roadmap.md)**。この表は「何があるか」、
  ロードマップは「どの順でやるか、なぜその順か」

最終更新: 2026-08-26

## 凡例

| 記号 | 意味 |
|---|---|
| ✅ | マージ済み |
| 🟡 | 着手可能（依存が解決済み） |
| ⬜ | 依存待ち |
| ❓ | 前提条件の判断待ち（耳での確認・測定など） |
| ❌ | 判断の結果やらないことにした（根拠は計画書に記録） |

## 現在地

**Doubler は一通り動いていて、見た目も含めて実機で確認済み**（2026-08-26）。
残りは下の「今すぐ着手できるもの」と「積み残し」だけ。

- 音: DSP 全単位完了。テスト 103 本
- UI: Voice Field・Filter View・Detail 表・ミラー編集・ツールチップ・
  キーボード操作・解析の重ね描き
- CPU: エンジン 70 µs + 解析 15 µs = **85 µs / 予算 533 µs**（1 コアの 0.8%）。
  Velour は **128 µs**（`VEL-16`）
- CI: `check`（PR と main への push）と `release`（`<plugin>-v<version>` タグ）
  の両方が動作確認済み。`doubler-v0.1.0` が下書き Release として出ている
- 共通クレート: `nxe-ui`（ウィジェット）と `nxe-dsp`（解析）。他プラグインで
  そのまま使える状態

**2 個目のプラグイン Velour の設計が始まった**（2026-08-26）。要件
（[`REQ-VEL`](../../plugins/velour/docs/requirements/REQ-VEL.md)）と
[DSP 仕様](../../plugins/velour/docs/specifications/dsp.md) と
[UI 仕様](../../plugins/velour/docs/specifications/ui.md)は書けている。
[実装計画](../../plugins/velour/docs/implementation/velour-plan.md)も書けている。
**`VEL-1` のゲートは通った**（`k` を振っても参照振幅でのレベルが ±0.3 dB 以内、
全 `(β, h)` で）。実装中に正規化の方式を差し替えていて、理由は `dsp.md` と
`velour-plan.md` に記録した。`VEL-2`（オーバーサンプラ）と `VEL-3`（帯域生成器）
も完了。最悪ケースの折り返しは **−63 dB**。

**`k` の上限は 2 回動いた**: 仕様の 20 → `VEL-2` の実測で 6 → `VEL-3` で AIR の
入力に上蓋を付けて 8。`REQ-VEL-020` が決めた順（入力を絞る → ドライブを絞る →
倍率は最後）そのまま。

**`VEL-5` で音が出て、Live で確認済み**（2026-08-26）。`VEL-4` で `TEXTURE`、
`VEL-9` で `Bias_i` / `Texture_i` / `Solo_i` が入り、**パラメータは 18 個**。

`VEL-8` で Guard が入り、**パラメータは 20 個**。

**`SOLO` が使えるようになった** — 並列生成でしか提供できない「足している層だけを
聴く」。

**`VEL-6` で `EMOTION` が入り、`VEL-7` で `DENSITY` が入って、パラメータは
22 個で打ち止め。** 2 つは検波器（`envelope.rs`、ピーク、モノ和、**圧縮前**）を
共有し、`DENSITY` が倍音の量を揃え `EMOTION` が質を選ぶという直交になっている。

**DSP も UI も一通り入った**（`VEL-15` まで）。主役の Band Field、伝達曲線の
小窓、IN / OUT のメーター、Advanced タブ、解析 2 本と Guard の表示。
**`VEL-16` で CPU を測った**（M シリーズ、release、512 サンプル / 48 kHz）:
エンジン 4x が **79 µs**、`Spectrum` 48 バンド × 2 本が **45 µs**、`Level` × 4 が
**4 µs** で、**合計 128 µs / 予算 533 µs**（目標 250 µs も達成）。削り順を
決めてあったが 1 つも要らなかった。**`EMOTION` のカーブ再構築はコストとして
測れない** — サンプルごとの仕事と 2 桁違うので、係数をブロック単位で作り直す
設計が正しかったことの確認になる。

**`VEL-17` で既定値を耳で決めた**: `DRIVE` 80 / `BODY` 40 / `PRESENCE` 60 /
`AIR` 40 / `TEXTURE` 50 / `DENSITY` 50 / `MIX` 80。仮の値より**前に出す方向**に
振れた（箱から出して何も起きないサチュレータは、趣味が良いのではなく壊れて
いるように読める）。**`DENSITY` は 0 → 50 で、構造からの議論を耳が引っくり返した** —
掛かっているのは声ではなく足している質感なので、常時少し入れておくと
「演奏が平らになる」のではなく「足す質感が揃う」になる。

**Velour は `velour-v0.1.0` で一区切り。** トリム 9 個と Guard のしきい値、
`REFERENCE_DB` は仮のまま残してある（触る理由が出てから動かす）。

**3 個目 Sparkleur の設計が済んだ**（2026-08-26）。要件
（[`REQ-SPK`](../../plugins/sparkleur/docs/requirements/REQ-SPK.md)）と
[DSP 仕様](../../plugins/sparkleur/docs/specifications/dsp.md)、
[UI 仕様](../../plugins/sparkleur/docs/specifications/ui.md)、
[実装計画](../../plugins/sparkleur/docs/implementation/sparkleur-plan.md)まで。
**実装は 1 行も書いていない。**

方向は「**VO-TT の代わり + 手軽に綺麗**」。5 帯域の上下コンプ（分割型 —
Velour の並列生成の裏返し）に、トランジェントでゲートした倍音生成を足す。
WIDTH と PUNCH は v2（`REQ-SPK-019` / `REQ-SPK-020`）。

設計で決めた主なもの:

- **分割は LR4 の木 + オールパス補正。** 「全部 unity で和が ±0.1 dB 以内に
  平坦」が `SPK-2` のゲート
- **per-band の Attack / Release を出さない。** `SPEED` 1 本 + 帯域の中心
  周波数から導出。100 Hz の 1 波長は 10 ms で、それより速い attack は波形を
  変調する — **「LOW だけ速く」はできてはいけない操作**
- **上げコンプに床と上限を機構として持つ。** 概念文書の Sub Protect は
  「LOW の上限だけ小さい」で済む
- **`SPARK` は「マクロのマクロ」にしない**（`REQ-VEL-019` が却下されたのと
  同じ形）。量そのものに落とし、質は `CHARACTER` の 1 軸
- **`MIX` = 0 だけがビット一致。** `SPARK` = 0 は振幅平坦・位相は回る
  （分割型の帰結を正直に要件に書いた）

`VEL-10` で**バグを 1 個潰した**: 非有限のサンプル 1 個で Guard がそのセッション
の間ずっと無効になる（検波器の状態が再帰的なので NaN が抜けない）。音は出続ける
ので他のテストに引っかからない類い。

**仕様の数字は 4 回動いた**: `k` の上限 20 → 6 → 8、`bias` のレベル補正
6 dB → 0、`DENSITY` のメイクアップの基準 full scale → `REFERENCE_DB`。
どれも実測が仕様を否定した結果で、理由は `dsp.md` と `velour-plan.md` にある。

Velour が共通クレートに要求したものは 3 つとも入っている（`UI-13` `BandField` /
`UI-8` `Meter` / `DSP-4` `Level`）。**Velour のクレートは 1 行も無い状態で
作れた** — 主役の図が Hz も dB も知らないから。テスト 124 本。

**`SPK-1` が終わった**（2026-08-26）。`velour-core` から `shaper` /
`oversample` / `biquad` / `envelope` / `guard` と、測定ヘルパの `harmonics` を
新クレート `crates/nxe-audio` に移し、`guard` を `RelativeGuard<N>` に
一般化した。**Velour のテストは 1 本も落ちていない** — 241 本が 241 本のまま
通り、`N = 1` の形を固定する 1 本を足して **242 本**。`envelope` は機構だけ
移して、attack 5 ms / release 150 ms と `REFERENCE_DB` は**歌に対して耳で決めた
数**なので `velour-core` に残した。**次は `SPK-2`**（分割の和が ±0.1 dB 以内で
平坦。ここが通らないとその上の全部が意味を持たない）。

## 今すぐ着手できるもの

依存が無いか、依存がすべて解決している単位。

| ID | 単位 | 計画 |
|---|---|---|
| SPK-2 | **クロスオーバー（ゲート）** — LR4 の木 + オールパス補正、5 帯域、`FOCUS` | `sparkleur-plan.md` |
| SPK-10 | `nxe_ui::band::Band` の `reduction` を符号付き `delta` に（上げコンプが描けない） | `sparkleur-plan.md` |
| SPK-11 | `param_bind` の共通化（3 個目が要求した。行き先は `nxe-ui` ではない） | `sparkleur-plan.md` |
| DBL-13 | 既定値の詰めと実機確認（フェーズ 4。**耳が要る**） | `doubler-plan.md` |
| UI-9 | `ToggleSwitch` — **3 個目の設計でも要らなかった**（`.segment` を当てた `Label` で足りる）。**落として良い** | `nxe-ui-plan.md` |

Velour が共通クレートに要求した 3 つ（`UI-13` / `UI-8` / `DSP-4`）は完了。
**gallery で見る**（`mise run gallery`）。

## 積み残し（どれも単位を持っていない小物）

`✅ ❓` が付いている単位の未完了部分。まとめて 1 単位にしてもよい。

| 何 | どこ |
|---|---|
| 値の直接入力を戻す | `UI-3`。`ValueEntry` は `nxe-ui` にあり gallery では動く。プラグインに載せると editor の表示が更新されなくなる（原因未特定） |
| 見た目の最終調整（フォントサイズ、寸法、余白） | ユーザーの指示で最後にまとめる |

## 全単位

### インフラ — `infra-plan.md`

| ID | 単位 | 状態 |
|---|---|---|
| INFRA-1 | ワークスペース骨格 | ✅ `3f98dce` |
| INFRA-2 | プルリクエストの CI（`mise run check`） | ✅ |
| INFRA-3 | リリースの CI（タグ → 3 OS bundle → Release） | ✅ `doubler-v0.1.0` で実行済み |

### 共通の解析 — `nxe-dsp-plan.md`

| ID | 単位 | 状態 |
|---|---|---|
| DSP-1 | 音声スレッド → UI の受け渡し（`Handoff`） | ✅ |
| DSP-2 | ステレオ像の分布（`PanScope`） | ✅ |
| DSP-3 | スペクトラム（`Spectrum`。定 Q のフィルタバンク） | ✅ |
| DSP-4 | レベル（`Level`。peak + RMS + ピークホールド） | ✅ |

### 共通 UI — `nxe-ui-plan.md`

| ID | 単位 | 状態 |
|---|---|---|
| UI-1 | テーマトークンと gallery の中身 | ✅ |
| UI-2 | Lucide アイコン埋め込みと定数生成 | ✅ |
| UI-3 | 共通の入力ふるまい（ドラッグ / 微調整 / リセット / 値入力 / ジェスチャー通知） | ✅ ❓ 値の直接入力は差し戻し |
| UI-4 | `Knob`（大小 2 サイズ） | ✅ |
| UI-5 | `Bar` | ✅ |
| UI-6 | `SegmentedControl` | ✅ |
| UI-7 | `PolarField`（領域知識を持たない極座標フィールド） | ✅ |
| UI-10 | `CurveView`（領域知識を持たない曲線表示） | ✅ |
| UI-11 | `PolarField` の分布オーバーレイ | ✅ |
| UI-12 | `CurveView` の解析カーブ | ✅ |
| UI-13 | `BandField`（領域知識を持たない帯域パネル） | ✅ |
| UI-8 | `Meter` — **Velour の IN / OUT が使う** | ✅ |
| UI-9 | `ToggleSwitch` — **誰も使っていない**（2 個目でも不要） | 🟡 |

### Doubler — `../../plugins/doubler/docs/implementation/doubler-plan.md`

| ID | 単位 | 状態 |
|---|---|---|
| DBL-1 | リングバッファ遅延線と Hermite 補間 | ✅ `a84779d` |
| DBL-2 | 回転タップピッチシフタ（**ゲート通過**） | ✅ `f2501d6` |
| DBL-3 | ボイスエンジン（形状表・実効値・パン則・ゲイン補償） | ✅ `3aec7af` |
| DBL-4 | nih-plug ラッパと配線（**ここで音が出る**。UI 無し） | ✅ `556c5a0` |
| DBL-5 | Humanize | ✅ |
| DBL-6 | Source モード（Mono Sum / True Stereo） | ✅ |
| DBL-7 | Tone と Tone Spread | ✅ |
| DBL-8 | スムージングとサンプルレート／ブロックサイズ非依存の詰め | ✅ |
| DBL-9 | UI マクロ層（ノブとセグメント） | ✅ |
| DBL-10 | UI Voice Field | ✅ |
| DBL-11 | UI Detail 層（ボイス表） | ✅ |
| DBL-14 | UI Filter View | ✅ |
| DBL-15 | UI ミラー編集（`REQ-DBL-014`） | ✅ |
| DBL-12 | CPU 予算の確認（criterion） | ✅ 69.4 µs / 予算 533 µs |
| DBL-16 | 通っている音の表示（`REQ-DBL-015`） | ✅ |
| DBL-13 | 既定値の詰めと実機確認 | 🟡 |

### Velour — `../../plugins/velour/docs/implementation/velour-plan.md`

**ゲートは `VEL-1`**（`k` を振っても基音が ±0.1 dB 以内）。ここが崩れると
4 層の直交（`REQ-VEL-009`）が成立せず、パラメータの意味づけと UI が全部
書き直しになる。ただし**耳ではなく単体テストで通る**。

| ID | 単位 | 状態 |
|---|---|---|
| VEL-1 | シェイパ（**ゲート**。`velour-core` クレートもここで作る） | ✅ **ゲート通過** |
| VEL-2 | オーバーサンプラ（2 段 halfband、2x / 4x） | ✅ 4x で折り返し −68 dB |
| VEL-3 | 帯域生成器 3 本と `FOCUS` | ✅ 最悪ケース折り返し −63 dB |
| VEL-5 | nih-plug ラッパと配線（**ここで音が出る**。UI 無し） | ✅ Live で確認済み |
| VEL-4 | TEXTURE モーフ（Warm / Clear / Edge） | ✅ |
| VEL-6 | エンベロープ検波と EMOTION | ✅ |
| VEL-7 | DENSITY（生成バスの圧縮） | ✅ |
| VEL-8 | Guard（Harsh / Sib） | ✅ |
| VEL-9 | Bias と SOLO | ✅ |
| VEL-10 | スムージングと非依存性の詰め | ✅ 非有限で Guard が固まるバグを 1 個潰した |
| VEL-11 | UI マクロ層（メインタブ 8 ノブ） | ✅ |
| VEL-12 | UI Band Field | ✅ |
| VEL-15 | 通っている音の表示（解析の配線） | ✅ |
| VEL-13 | UI 伝達曲線の小窓と I/O メーター | ✅ |
| VEL-14 | UI Advanced タブ | ✅ |
| VEL-16 | CPU 予算の確認（criterion） | ✅ 128 µs / 予算 533 µs |
| VEL-17 | 既定値の詰めと実機確認（**耳が要る**） | ✅ |

### Sparkleur — `../../plugins/sparkleur/docs/implementation/sparkleur-plan.md`

**ゲートは `SPK-2`**（全帯域 unity のとき 20 Hz〜20 kHz の和が ±0.1 dB 以内で
平坦）。ここが通らないと「全部 0 で何もしない」が成立せず、その上の全部が
意味を持たない。Velour の `VEL-1` と同じ位置。

`SPK-10` と `SPK-11` は共通クレート側の単位で、Sparkleur のコードを 1 行も
書かずに着手できる。

| ID | 単位 | 状態 |
|---|---|---|
| SPK-1 | 共有クレート `nxe-audio`（`velour-core` から 6 モジュール移動 + `guard` の一般化） | ✅ `2817adc`〜`38ae534` |
| SPK-2 | クロスオーバー（**ゲート**。LR4 の木 + オールパス補正、`FOCUS`） | 🟡 |
| SPK-3 | 検波と時定数（`SPEED` と帯域中心からの導出） | ⬜ SPK-2 |
| SPK-4 | ゲイン計算（上下コンプ。**製品の核**） | ⬜ SPK-3 |
| SPK-5 | CHARACTER（POLISH ↔ CRUSH の 1 軸） | ⬜ SPK-4 / SPK-6 |
| SPK-6 | Sparkle（トランジェントでゲートした倍音生成） | ⬜ SPK-2 |
| SPK-7 | De-Harsh / Sub Protect | ⬜ SPK-4 / SPK-5 |
| SPK-8 | ラッパとパラメータ（**ここで音が出る**） | ⬜ SPK-4 / SPK-5 / SPK-6 / SPK-7 |
| SPK-9 | 詰め（レート・ブロック・極端値） | ⬜ SPK-8 |
| SPK-10 | `nxe_ui::band::Band` の `reduction` を符号付き `delta` に | 🟡 |
| SPK-11 | `param_bind` の共通化（行き先は `nxe-ui` ではない） | 🟡 |
| SPK-12 | UI マクロ層（メインタブ） | ⬜ SPK-9 / SPK-11 |
| SPK-13 | UI Band Field（5 区画） | ⬜ SPK-10 / SPK-12 / SPK-16 |
| SPK-14 | UI 小窓とメーター | ⬜ SPK-12 / SPK-16 |
| SPK-15 | UI Advanced タブ | ⬜ SPK-13 |
| SPK-16 | 解析の配線 | ⬜ SPK-8 |
| SPK-17 | CPU 予算 | ⬜ SPK-16 |
| SPK-18 | 既定値と耳 | ⬜ SPK-17 |
