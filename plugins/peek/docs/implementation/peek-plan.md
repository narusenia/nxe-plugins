# Peek 計画 — メーター 7 本と、まとめの Peek

> 2026-11-25 に計画。ラインナップと順番の理由は
> [`series-plan.md`](../../../../docs/implementation/series-plan.md) の「Peek」。
> ここは単位と、出荷後に変えられないものを持つ。
> **Peek は Kit ではなく独立したシリーズ**（2026-11-25）。窓は Kit の型を借りる。

## 決めたこと

1. **どれも音を変えない。** 出力は入力とビット一致、レイテンシは 0。
   計測は出力を作り終えた後の入力を読むだけ
2. **計測は `peek-core` に 1 つだけ置く。** 7 本が同じ計測を使う。Kit と Peek で
   同じ数字が違って見えることがないように、計測の実装は 1 か所にする。
   プラグインのクレート（`peek-level` など）は宣言と窓だけを持つ
3. **窓は Kit の型（Kit 7、`design-system.md`）を `nxe-egui::kit` に置く。**
   Peek Level が最初の利用者で、寸法はここで決める。Kit 11 本と Peek の 7 本が
   同じ型を使う。左の縦帯は `<シリーズ> KIT`（Peek も `PEEK KIT`）
4. **差し色は無彩色。** `Palette::PEEK`。隣に挿したプラグインの色と競わない。
   Basic の色の家族（`Palette::ALL`、色相を 40° ずつ離す試験）には入れない
5. **Kit 7 の下の帯は、メーターではつまみではなく読み値を並べる。** 下の帯は
   「操作の帯」ではなく「L の下の辺」として扱う。パラメーターの無い Kit は
   読み値を置く
6. **最大値のリセットは窓からの要求で、音声スレッドが行う。** 窓は要求の回数を
   数える原子変数を 1 つ増やすだけで、計測の状態には触れない
7. **トゥルーピークは `nxe-audio` の 2 倍オーバーサンプラを 2 段**（4 倍）。
   BS.1770 の参照フィルタ（48 タップの FIR）ではないので、表示は「規格準拠」と
   言わない。誤差は試験で測って下に書く

## 出荷後に変えられないもの

`truce.toml` に書く。名前は `Peek <計測>`、クレートは `peek-<計測>`。

| プラグイン | `bundle_id` | AU `fourcc` | クレート |
|---|---|---|---|
| Peek Level | `peeklevel` | `PkLv` | `peek-level` |
| Peek VU | `peekvu` | `PkVu` | `peek-vu` |
| Peek Stereo | `peekstereo` | `PkSt` | `peek-stereo` |
| Peek Spectrum | `peekspectrum` | `PkSp` | `peek-spectrum` |
| Peek Loudness | `peekloudness` | `PkLd` | `peek-loudness` |
| Peek Scope | `peekscope` | `PkSc` | `peek-scope` |
| Peek Spectrogram | `peekspectrogram` | `PkSg` | `peek-spectrogram` |
| Peek | `peek` | `Peek` | `peek` |

**全部ここで決める。** Basic の V2-1 と同じ理由で、急いで決めるものを残さない。
商標と既存製品との重複は確認していない。

## 単位

| ID | 単位 | 依存 |
|---|---|---|
| PEEK-1 | Kit の窓の型（`nxe-egui::kit`）、`peek-core` の Level とトゥルーピーク、Peek Level | — |
| PEEK-2 | Peek VU | PEEK-1 |
| PEEK-3 | Peek Stereo | PEEK-1 |
| PEEK-4 | Peek Spectrum | PEEK-1 |
| PEEK-5 | Peek Spectrogram（`nxe-spectral` を切り出す） | PEEK-4 |
| PEEK-6 | Peek Loudness（BS.1770 の K 特性とゲーティング、EBU の試験信号） | PEEK-1 |
| PEEK-7 | Peek Scope | PEEK-1 |
| PEEK-8 | Peek（7 本の計測を並べた大きな窓） | PEEK-2〜7 |
| PEEK-9 | 最初の beta（`peek-<YYYY.MM>-beta.N`） | PEEK-1 |

PEEK-9 は Peek Level だけでも切れる。Kit の型を実機で確かめるのが先。

### PEEK-1 — Peek Level

見るもの:

- チャンネルごと（L / R、モノなら 1 本）の横棒。サンプルピーク（速い落ち）、
  その中に RMS、1.5 秒のホールドの印
- **−60 dBFS から +3 dBFS。** 0 dBFS を越えるトゥルーピークが見えるように、
  上に 3 dB の余白を取る
- 下の帯に、クリアしてからの最大値: サンプルピーク、トゥルーピーク、RMS。
  0 dBFS を越えたトゥルーピークは目立たせる。帯をクリックするとクリア

完了条件:

- 出力が入力とビット一致（試験）
- フルスケールの正弦波の RMS が −3.01 dB、ピークが 0 dB（試験）
- サンプル間でピークを取る正弦波（fs/4、位相 45°）で、サンプルピークは
  −3 dB、トゥルーピークは 0 dB から ±0.5 dB 以内（試験）
- 音声スレッドで確保しない（`rt-paranoid`）
- 基準画像（macOS / Windows、CI で焼く）
- validate（CLAP / VST3 / AU）

**パラメーターは無い。** プリセットの帯も置かない。truce はパラメーターが 0 個の
構造体を受け付けないので、L / R のピークをホストへ返すメーター枠（`#[meter]`）を
2 つ宣言している。オートメーションできるものは無い。

### PEEK-2 — Peek VU

**見た目を切り替えられる。** そのうちの 1 つは、針が弧の上を振れるアナログの
VU（2026-11-25 にユーザーが見本を出した）:

- 目盛りは −20 から +3 VU。0 より上は弧の帯の色を変える
- 針は 1 本で、L と R の大きい方を指す。軸の上に `VU`
- 下の左右に L / R の読み値（dB）、その下に −1〜+1 の横棒（相関）

**デザインの規則と 2 か所ぶつかる**（`design-system.md`）:

1. 曲線は信号だけに使う規則。弧の目盛りは記号なので、この規則の外になる
2. 1 つの窓に色相は 1 つの規則。0 より上の帯と針に、差し色とは別の色を使っている

どちらも、作る前に例外として認めるか、直線と無彩色に寄せるかを決める。

### PEEK-5 — Peek Spectrogram

横が時間、縦が周波数（対数）、濃さが大きさ。新しい列が右から入って左へ流れる。

- **STFT は Pumice のものを `nxe-spectral` に切り出して使う**（series-plan の Spectral の Kit と同じ切り出し）。
  窓に送るのは 1 フレームずつの大きさで、`Handoff` の「最新だけ」では列が抜けるので、
  決まった数のフレームを持つリングバッファで渡す
- 窓は 1 列ずつ書き足すテクスチャ。全面を毎回描き直さない
- 色は差し色の濃淡だけ（1 窓 1 色相の規則のまま）。濃さの下限と上限の dB は窓で変えられる
