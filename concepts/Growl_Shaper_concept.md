# Growl Shaper Concept

## Overview

Dubstep / Riddim / Neuro / Bass Music向けのBass Character Processor。

コンセプトは、

> **Bassに“発音器官”を与える。**

単なるFormant FilterやDistortionではなく、

- Mouth
- Throat
- Teeth
- Snarl
- Talk

という身体的なメタファーで、Bassの「喋り方」を操作する。

---

# Core Signal Flow

```text
Input
  ↓
Pre Saturation
  ↓
Formant Bank A
  ↓
Nonlinear Waveshaper
  ↓
Formant Bank B
  ↓
Dynamic Resonance Control
  ↓
Texture / Teeth Generator
  ↓
Post Clip
  ↓
Output
```

重要なのは、

```text
Filter → Distortion → Filter
```

という構造。

Formantで作ったピークを歪ませ、その倍音を再度Formant処理することで複雑な発音感を生成する。

---

# MOUTH

中心となるMacro。

Bassの母音・口の形をMorphさせる。

例：

```text
OO → OH → AH → EH → EE
```

ただし人間の母音を完全再現するだけでなく、Bass向けに誇張された非現実的なFormantセットも用意する。

MOUTHは複数のBandpass / Peak Filterを同時にMorphする。

---

# THROAT

声道サイズのような感覚を操作。

```text
THROAT ↓
Deep / Monster / Large

THROAT ↑
Small / Alien / Screech
```

内部的には、

- Formant frequency shift
- Spectral tilt
- Resonance spacing
- Harmonic weighting

を連動させる。

---

# SNARL

攻撃性。

SNARLを上げると、

- Formant Q ↑
- Saturation ↑
- Odd harmonics ↑
- Resonance emphasis ↑
- Midrange bite ↑

する。

高Resonanceで暴れすぎないよう、

- Dynamic notch
- Peak limiting
- Resonance-aware gain compensation

を内部で行う。

---

# TEETH

高域の歯擦音・Noise成分。

Bassに、

- krr
- zzz
- shk
- tss

のような質感を加える。

基本構造：

```text
Noise Generator
      ↓
Bandpass / Comb
      ↓
Input Envelope Follower
      ↓
Texture Shaping
      ↓
Mix
```

元信号のEnvelopeに追従させることで、独立したNoiseではなくBassの発音成分として聞こえる。

---

# TALK

MOUTHを自動変調するMacro。

Source候補：

- LFO
- Envelope Follower
- Transient
- Sidechain
- MIDI Velocity
- MIDI Note
- Step Sequencer

TALKを上げるほどMOUTHの変化幅が増える。

---

# Motion Engine

## LFO

- Free
- Sync
- Sine
- Triangle
- Saw
- Random
- Smooth Random

## Envelope

入力Bassの強弱に応じてMOUTH / SNARLを変化。

例：

```text
Quiet
→ OO

Loud
→ AH / EE
```

これにより演奏強度に応じて喋り方が変わる。

---

# Step Sequencer

8〜16 Step程度のMOUTH Sequencer。

例：

```text
OO | AH | EE | AH | OH | EE | OO | AH
```

同期候補：

- 1/4
- 1/8
- 1/16
- Triplet
- Dotted

各Stepで設定可能：

- Mouth position
- Snarl
- Throat
- Gate
- Accent

---

# Formant Sets

## Human

OO / OH / AH / EH / EE系。

## Monster

低いFormant、広い共鳴。

## Alien

高いFormant、狭いResonance。

## Machine

Comb / metallic peakを混ぜる。

## Screech

High-midに極端なFormant。

---

# Dynamic Resonance Control

Formant Filterは高Q時にPeakが極端になりやすい。

そのため各Formant帯域を監視し、

```text
Resonance ↑
Peak Detector ↑
Dynamic Gain Reduction ↑
```

する。

音色は保ちながら、耳に痛いピークを制御する。

---

# Main Controls

| Control | Role |
|---|---|
| MOUTH | 母音 / Formant Morph |
| THROAT | 声道サイズ |
| SNARL | 攻撃性・Resonance |
| TEETH | 高域Texture |
| TALK | 自動発音量 |
| DRIVE | Nonlinear processing |
| MIX | Dry / Wet |

---

# Advanced

## Formant

- Formant positions
- Q
- Gain
- Morph curve

## Motion

- LFO
- Envelope
- Sequencer
- Sidechain

## Distortion

- Soft clip
- Fold
- Rectify
- Hard clip
- Asymmetric

## Safety

- Resonance control
- Output clip
- Auto gain

---

# Design Philosophy

Growl Shaperは、

> **BassにEQやFilterを掛けるのではなく、Bassに「口」「喉」「歯」を与える。**

ユーザーはHzやQではなく、

- どんな口で
- どんな喉で
- どれだけ牙を剥いて
- どう喋らせるか

を操作する。
