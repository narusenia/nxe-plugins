# シリーズ計画 — v2 の後に何を出すか

> 2026-11-24 に決めたもの。窓の形は
> [`../specifications/design-system.md`](../specifications/design-system.md) の
> 「シリーズと強度」が持つ。ここはラインナップと順番だけを持つ。
> 名前の商標、既存製品との重複、DSP が実現できるかはまだ確認していない。

## シリーズ

| シリーズ | 何のためか | 本数 |
|---|---|---|
| **Basic** | ボーカルとミックス。v2 で出す 6 本（Doubler / Velour / Sparkleur / Air / Diorama / Pumice） | 6 |
| **Colour** | 音に色を足す。和声、共鳴、にじみ | フラッグシップ 4 |
| **Design** | 音を作り変える。寄生、ゆがみ、崩し | フラッグシップ 5 |
| **Create** | 音源 | フラッグシップ 3 |
| **Kit** | フラッグシップの 1 機能を、つまみ 2〜4 個で出す小さなプラグイン | 11 |

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

## 順番

1. **V2 を終える**（Basic の 6 本）
2. **Kit 2 組と、共通エンジンの切り出し**（`nxe-spectral`、Nonlinear）。リスクが
   低く、すぐ出せる
3. **Parasite**。新しく要るのはピッチ追従だけで、これが Harmonic エンジンの
   最初の部品になる
4. **Meniscus と Prisma**。Resonance エンジン（減衰振動、結合共振）を作る。
   Kit の Dew と Halo を先に出す
5. **Diffract**。Spectral と Harmonic に、MIDI とサイドチェインを足す。Colour の看板
6. **Create**（Swarm、次に Iridesce）。音源の基盤（ボイス管理、MIDI）がまだ
   無いので最後
