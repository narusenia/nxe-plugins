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
| **Peek** | メーターの Kit。音を変えず、見るだけ（下の「Peek」） | 5 |

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

## Peek — メーターの Kit

iZotope Insight や MiniMeters のような計測を、**1 つの窓に全部並べるのではなく、
1 つの計測を 1 本の Kit として**出す。必要なものだけトラックに挿せて、窓も
小さく済む。Kit と同じく 1 本ずつ別のプラグインで、窓は Kit の型を使う。

**どれも音を変えない。** 出力は入力とビット一致、レイテンシは 0。計測の
処理は音声スレッドで走るが、描画用の値は `nxe-dsp` の `Handoff` で渡すだけで、
窓が閉じていれば描画の費用は 0。

| Kit | 何を見るか | 作るもの |
|---|---|---|
| **Peek Level** | ピーク・RMS・トゥルーピーク（チャンネルごと、ホールド付き） | `nxe-dsp::Level` はある。トゥルーピークは 4 倍オーバーサンプリング（`nxe-audio` の 2 倍を 2 段） |
| **Peek Loudness** | LUFS（モーメンタリ / ショートターム / インテグレーテッド）、LRA、目標値との差、履歴のグラフ | ITU-R BS.1770 の K 特性（`nxe-audio` の biquad）とゲーティング。**新しく書く部分がいちばん多い** |
| **Peek Spectrum** | スペクトル（ピークホールド、傾き補正） | `nxe-dsp::Spectrum` はある。細かい分解能が要れば `nxe-spectral`（Pumice の STFT を切り出したもの）を使う |
| **Peek Stereo** | ベクトルスコープ、相関、左右のバランス | `nxe-dsp::Correlation` と `PanScope` はある |
| **Peek Scope** | 波形（オシロスコープ）と、流れる波形の履歴 | 新しく書く。周期に同期させるのは後 |

順番は Level → Stereo → Spectrum → Loudness → Scope。部品がそろっているものから
出して、Kit の窓の型をメーターで先に固める。Loudness は規格どおりかを確かめる
試験（EBU の試験信号）が要るので、手前の 3 本で型が決まってから。

**リリースは `series.toml` の `peek` シリーズ**（`peek-<YYYY.MM>`）。差し色は
仮に無彩色（白）で、隣に挿したプラグインの色と競わないようにする。

まだ決めていないこと:

- プラグイン名を `NXE Peek Level` のように Peek を前に付けるか（クレートは
  `peek-level`）。商標と既存製品との重複は確認していない
- 全部を 1 本にまとめた版も出すか

## 順番

1. **V2 を終える**（Basic の 6 本）
2. **Peek と、Kit 2 組と、共通エンジンの切り出し**（`nxe-spectral`、Nonlinear）。
   リスクが低く、すぐ出せる。Peek の最初の 3 本は既存の `nxe-dsp` で作れるので、
   Kit の窓の型を固めるのにも使う
3. **Parasite**。新しく要るのはピッチ追従だけで、これが Harmonic エンジンの
   最初の部品になる
4. **Meniscus と Prisma**。Resonance エンジン（減衰振動、結合共振）を作る。
   Kit の Dew と Halo を先に出す
5. **Diffract**。Spectral と Harmonic に、MIDI とサイドチェインを足す。Colour の看板
6. **Create**（Swarm、次に Iridesce）。音源の基盤（ボイス管理、MIDI）がまだ
   無いので最後
