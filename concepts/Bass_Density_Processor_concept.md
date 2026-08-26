# Bass Density Processor Concept

## Overview

EDM / Bass Music向けのBass専用Density Processor。

コンセプトは、

> **Subを守りながら、Bassの知覚上のサイズと密度だけを巨大化する。**

一般的なOTTやMultiband Saturationでは、Bass全体を強く処理することで、

- Subが暴れる
- 低域のピークが不安定になる
- ノイズフロアまで持ち上がる
- Transientが潰れる
- Mid Bassだけ欲しいのに全帯域が歪む

といった問題が起こりやすい。

Bass Density Processorでは、帯域ごとに役割を分離し、低域の安定性を保ったままBODY / GROWL / BITEの密度を増やす。

---

# Core Signal Flow

```text
Input
 │
 ├──────── Analysis / Detector
 │
 ▼
3〜4 Band Split
 │
 ├─ SUB
 │    ├ Clean Compression
 │    ├ Peak Control
 │    ├ Mono / Phase Protection
 │    └ Sub Reconstruction
 │
 ├─ BODY
 │    ├ Soft Saturation
 │    ├ Upward Compression
 │    └ Density Compression
 │
 ├─ GROWL
 │    ├ Strong Saturation
 │    ├ Parallel Clipping
 │    └ Harmonic Enhancement
 │
 └─ BITE / AIR
      ├ Transient Emphasis
      └ Exciter
 │
 ▼
Recombine
 │
 ├ Transient Restoration
 ├ Dynamic Spectral Balance
 └ Output Clipper
 │
 ▼
Output
```

---

# DENSITY

メインMacro。

DENSITYを上げると、

```text
Upward Compression ↑
Saturation ↑
Parallel Compression ↑
Crest Factor ↓
Transient Restore ↑
```

を連動させる。

目的：

> **Denseなのに、潰れて死んでいないBass。**

Transient Restoreは元信号のAttack成分を解析し、過度なCompressionで失われたアタックを再注入する。

---

# SUB SAFE

Bass Density Processorの重要機能。

低域では過剰なUpward CompressionやSaturationを避ける。

例：

```text
Below 80〜120 Hz

Upward Compression ↓ strongly
Saturation ↓
Stereo Width → Mono
Peak Control ↑
Phase Protection ↑
```

クロスオーバー周波数は調整可能。

狙い：

> **Mid / High Bassは壊すが、Subは壊さない。**

---

# Sub Reconstruction

処理によって低域バランスが崩れた際に、元信号のSub成分を再利用する。

```text
Original Input
   ↓ LPF
Original Sub ─────────────┐
                          ├→ Recombine
Processed Mid / High ─────┘
```

SUBノブは単なるEQ Gainではなく、

```text
Processed Sub ←→ Original Sub
```

のMorphとしても使える。

これにより、激しく歪ませても安定した低域を維持できる。

---

# BODY

主に100〜400 Hz付近。

目的：

- Bassの肉厚感
- 小型スピーカーで感じる重量
- Low-mid sustain
- 音像の物理的サイズ感

処理候補：

- Even harmonic saturation
- Parallel compression
- Low-mid sustain shaping
- Dynamic low-mid balance

---

# WEIGHT

単純なBass Boostではない。

WEIGHTは、

- 120〜300 Hzの倍音
- Even harmonics
- Low-mid sustain
- Sub / Mid balance

を統合操作する。

目的：

> **低音が出ない再生環境でも、重さを感じさせる。**

---

# GROWL

中域の攻撃性と倍音密度を操作する。

主な処理：

- Waveshaping
- Parallel clipping
- Upward compression
- Harmonic emphasis
- Midrange sustain

Growl ShaperほどFormant的な動きは持たず、こちらでは密度と破壊感に集中する。

---

# BITE

主に1〜5 kHz。

目的：

- BassをMix内で聞こえやすくする
- Attackの輪郭を出す
- Midrangeの可聴性を上げる

BITEを上げると、

```text
Upper Harmonics ↑
Transient Edge ↑
Presence ↑
High-mid Compression ↓ slightly
```

する。

---

# PUNCH

Compression後に失われやすいAttack成分を復元。

入力からTransient envelopeを抽出し、

```text
Original Transient
      ↓
Transient Extractor
      ↓
Gain / Shape
      ↓
Processed Bassへ再注入
```

する。

---

# Main Controls

| Control | Role |
|---|---|
| DENSITY | 全体の密度 |
| WEIGHT | 重量感 |
| GROWL | 中域の倍音・攻撃性 |
| BITE | 高中域の抜け |
| PUNCH | Transient Restore |
| SUB | Original / Processed Sub balance |
| MIX | Dry / Wet |

Additional:

- SUB SAFE
- Output Clip
- Crossover Frequency

---

# Character Modes

## Clean

- Sub protection強め
- Soft saturation
- Transient保持
- Modern EDM Bass向け

## Heavy

- Body / Weight強め
- Parallel compression増加
- Dubstep / Bass House向け

## Destroy

- Growl saturation強め
- Parallel clipping
- Upward compression増加
- Riddim / Experimental Bass向け

---

# Design Philosophy

ユーザーが操作したいのはCompression Ratioではなく、

> **Bassをどれだけ巨大で密に感じさせたいか。**

そのためBass Density Processorは、

- Subの安定性
- Mid Bassの密度
- Harmonic weight
- Transient clarity
- Audible bite

を一つの音楽的な処理としてまとめる。
