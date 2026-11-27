# シリーズ計画 — v2 の後に何を出すか

> 2026-11-24 に決めたもの。窓の形は
> [`../specifications/design-system.md`](../specifications/design-system.md) の
> 「シリーズと強度」が持つ。ここはラインナップと順番だけを持つ。
> 元の案（約 80 案、各案の DSP とパラメーター）は
> [`../../concepts/series-ideas.md`](../../concepts/series-ideas.md)。
> 名前の商標、既存製品との重複、DSP が実現できるかはまだ確認していない。

## シリーズ

| シリーズ | 何のためか | 本数 |
|---|---|---|
| **Basic** | ボーカルとミックス。v2 で出す 6 本（Doubler / Velour / Sparkleur / Air / Diorama / Pumice） | 6 |
| **Colour** | 音に色を足す。和声、共鳴、にじみ | フラッグシップ 4 |
| **Design** | 音を作り変える。寄生、ゆがみ、崩し | フラッグシップ 5 |
| **Create** | 音源 | フラッグシップ 3 |
| **Kit** | フラッグシップの 1 機能を、つまみ 2〜4 個で出す小さなプラグイン | 11 |
| **Peek** | メーター。音を変えず、見るだけ。1 つの計測を 1 本にしたものと、全部をまとめた Peek（下の「Peek」）。窓は Kit の型を借りる | 7 + まとめ 1 |

## 2 つの軸（2026-11-26）

**シリーズは用途で分け、大きさはシリーズをまたぐ別の軸にする。**

- 用途: Basic（声にもミックスにも使える、迷わない実用品）、Colour（色を足す）、
  Design（作り変える）、Create（音源）、Peek（見るだけ）
- 大きさ: **Kit**（1 機能、つまみ 2〜4 個）← **フラッグシップ** → **統合版**
  （Colour と Design は Mutate / Reform）
- エンジンはシリーズをまたいで共有する。`resonance-core` が Colour の Kit と
  フラッグシップを兼ねているのと同じ形

**シリーズは後から組み替えられる。** プラグイン ID は `truce.toml` のプラグイン
ごとに決まり、シリーズが決めるのはリリースのまとまりと窓の型だけ。後から
変えられないのはプラグイン名と ID なので、先に決めるのは名前の重なり。

### Kit のリリースを家族で分ける

今は Kit の全 10 本で 1 つのリリース（`kit`）。Kit が増えると
数十本になり、インストーラーの一覧も長くなる。**15 本を超えたら、`series.toml` を
家族ごとに分ける**（`kit-nonlinear`、`kit-resonance` など）。プラグイン ID は
変わらないので、分けても使っている人のセッションは壊れない。

## フラッグシップと、そこへまとめた案

| シリーズ | フラッグシップ | まとめた案 | 切り出す Kit |
|---|---|---|---|
| Colour | **Diffract** | Chroma、Prismatica（帯域ごとの和音化をモードに）、Synesthesia（特徴量で和声を動かす部分をモジュレーションに） | — |
| Colour | **Prisma** | Halo、Mycelia（結合共振器のモード）、Aether のフリーズ | Halo |
| Colour | **Meniscus** | Liquefy | Dew |
| Colour | **Effloresce** | Flora、Pollen | Glint、Pollen |
| Design | **Parasite** | Entangle | — |
| Design | **Singularity** | Warp（カーブ編集モード）、Shear、Invert | Invert、Shear |
| Design | **Dislocate** | Fracture、Afterimage | Afterimage |
| Design | **Devour** | Hysteresis（中核の非線形）、Fold、Fever、Buckle、Corrode、Rupture | Fold、Fever、Buckle |
| Design | **Topology** | Torque、Symmetry | Torque |
| Create | **Swarm** | — （Quanta は粒子で別物なので分ける） | — |
| Create | **Iridesce** | — | — |
| Create | **Rhizome** | Bloom | — |

後回しにするもの:

- Mutate と Reform。上のエンジンがそろってからの上位製品
- Morph / Mimic、Formant / Articulate / Phoneme、Cymatica / Alloy / Glacia、
  Attractor / Anomaly、Chimera、Capillary。第 2 波
- Impact と Kinetic。Basic に入れる候補

## Kit

**Kit は 1 本ずつ別のプラグインとして出す。** 1 本の中で切り替える形にはしない。
プラグイン ID は出荷後に変えられないので、ここは最初に固定する。

Kit を出す理由は 3 つ。

1. エンジンを小さく出荷して、実地で確かめる。Fold、Fever、Buckle で Devour の
   非線形部を固める
2. フラッグシップより先に、名前と音を出しておく
3. 窓は全 Kit で同じ型を使う（design-system.md の Kit の型）。UI のコストを
   ほぼ 0 にする

最初の 2 組:

- **Spectral の Kit** — Invert、Shear、Afterimage。`pumice-core` の STFT を
  `nxe-spectral` として切り出して作る
- **Nonlinear の Kit** — Fold、Fever、Buckle。`nxe-audio` のオーバーサンプラと
  シェイパーで作る

## Peek — メーターのシリーズ

iZotope Insight や MiniMeters のような計測。**Kit ではなく、独立したシリーズ**
（2026-11-25 に決定）。**1 つの計測を 1 本のプラグインとして**出し（`Peek VU`、
`Peek Loudness` …）、**全部をまとめて見られるものを `Peek`** にする。要るものだけ
挿したいときは 1 本ずつ、全体を見たいときは Peek。1 本ずつのプラグインは
窓に Kit の型（Kit 7）を借り、左の縦帯も Kit と同じく `PEEK KIT`。Peek は同じ計測と
同じ描画部品を並べた大きな窓で、計測の中身は 1 本ずつのものと共有する（片方だけ
直すと数字が食い違うので、計測は共通のクレートに置く）。

**どれも音を変えない。** 出力は入力とビット一致、レイテンシは 0。計測の
処理は音声スレッドで走るが、描画用の値は `nxe-dsp` の `Handoff` で渡すだけで、
窓が閉じていれば描画の費用は 0。

| プラグイン | 何を見るか | 作るもの |
|---|---|---|
| **Peek Level** | ピーク・RMS・トゥルーピーク（チャンネルごと、ホールド付き） | `nxe-dsp::Level` はある。トゥルーピークは 4 倍オーバーサンプリング（`nxe-audio` の 2 倍を 2 段） |
| **Peek VU** | VU メーター（針、300 ms の積分、基準レベルを選べる） | 新しく書く。積分は小さい。見た目は針 1 本 |
| **Peek Loudness** | LUFS（モーメンタリ / ショートターム / インテグレーテッド）、LRA、目標値との差、履歴のグラフ | ITU-R BS.1770 の K 特性（`nxe-audio` の biquad）とゲーティング。**新しく書く部分がいちばん多い** |
| **Peek Spectrum** | スペクトル（ピークホールド、傾き補正） | `nxe-dsp::Spectrum` はある。細かい分解能が要れば `nxe-spectral`（Pumice の STFT を切り出したもの）を使う |
| **Peek Stereo** | ベクトルスコープ、相関、左右のバランス | `nxe-dsp::Correlation` と `PanScope` はある |
| **Peek Scope** | 波形（オシロスコープ）と、流れる波形の履歴 | 新しく書く。周期に同期させるのは後 |
| **Peek Spectrogram** | スペクトログラム（時間 × 周波数、濃さが大きさ） | `nxe-spectral` の STFT。窓は 1 列ずつ書き足すテクスチャ |

順番は Level → VU → Stereo → Spectrum → Spectrogram → Loudness → Scope、最後に Peek。部品が
そろっているものから出して、Kit の窓の型をメーターで先に固める。Spectrogram は Spectrum と同じ STFT を使うので、その直後。Loudness は
規格どおりかを確かめる試験（EBU の試験信号）が要るので、手前で型が決まってから。
Peek は 7 本の計測と描画部品がそろってから、それを並べて作る。

**リリースは `series.toml` の `peek` シリーズ**（`peek-<YYYY.MM>`）。差し色は
仮に無彩色（白）で、隣に挿したプラグインの色と競わないようにする。

名前は `Peek <計測>`（クレートは `peek-vu` など）と `Peek`
（クレートは `peek`）。商標と既存製品との重複は確認していない。

## 順番

1. **V2 を終える**（Basic の 6 本）
2. **Peek シリーズと、Kit 2 組と、共通エンジンの切り出し**（`nxe-spectral`、Nonlinear）。
   リスクが低く、すぐ出せる。Peek の最初の 3 本は既存の `nxe-dsp` で作れるので、
   Kit の窓の型を固めるのにも使う
3. **Parasite**。新しく要るのはピッチ追従だけで、これが Harmonic エンジンの
   最初の部品になる
4. **Meniscus と Prisma**。Resonance エンジン（減衰振動、結合共振）を作る。
   Kit の Dew と Halo を先に出す
5. **Diffract**。Spectral と Harmonic に、MIDI とサイドチェインを足す。Colour の看板
6. **Create**（Swarm、次に Iridesce）。音源の基盤（ボイス管理、MIDI）がまだ
   無いので最後
