# Air Texture Generator Concept

## Overview

EDM / Bass Music / Pop / Hyperpop向けの高域Texture Generator。

コンセプトは、

> **元音に存在していない“空気・粒子・高域テクスチャ”を、元信号から生成する。**

一般的なExciterは既存の高域倍音を強調することが中心だが、Air Texture Generatorでは、

- Harmonic Air
- Noise
- Fizz
- Breath
- Stereo particles

などの新しい高域成分を生成し、入力信号のEnvelopeやBrightnessに追従させる。

Sparkluerが「元音を磨く」なら、Airは

> **元音の表面そのものを作る。**

---

# Core Signal Flow

```text
Input
 │
 ├──────── Analysis
 │          ├ Envelope
 │          ├ Brightness
 │          ├ Transient
 │          └ Tonality
 │
 ├─ SHINE Generator
 ├─ DUST Generator
 ├─ FIZZ Generator
 └─ BREATH Generator
 │
 ▼
Follow / Modulation Engine
 │
 ▼
Stereo Texture
 │
 ▼
Dynamic Harshness Control
 │
 ▼
Mix
```

---

# AIR

中央のメインMacro。

AIRを上げると、選択されたCharacterに応じてTexture成分を増やす。

単純なHigh Shelfではなく、

> **高域成分の生成量**

を操作する。

---

# SHINE

倍音系Air。

```text
Input
 ↓ HPF
Nonlinear Generator
 ↓ HPF
Dynamic Control
 ↓
Mix
```

特徴：

- Clean
- Glossy
- Harmonic
- Tonal

用途：

- Lead
- Supersaw
- Vocal chop
- Pluck
- Synth

---

# DUST

Noise系Texture。

元信号のEnvelopeにNoiseを追従させる。

```text
Input Envelope
█████░░░░░

Generated Dust
████░░░░░░
```

用途：

- Snare
- Clap
- Percussion
- Bass transient
- Lo-fi / granular texture

DUSTはWhite Noiseだけでなく、

- Pink
- Blue
- Filtered
- Grain
- Vinyl-like

などのCharacterを持てる。

---

# FIZZ

攻撃的な高域。

生成例：

```text
Input
 ↓
Bit Crush / Fold / Clip
 ↓
High-pass
 ↓
Dynamic Gate
 ↓
Mix
```

特徴：

- Dirty
- Digital
- Aggressive
- Hyper

用途：

- Dubstep Bass
- Colour Bass
- Hyperpop
- Distorted Lead
- Drum Bus

---

# BREATH

滑らかなNoise / Air成分。

Pink / White Noiseを入力信号に追従させる。

制御Source：

- Envelope
- Spectral centroid
- Transient strength
- Input brightness

用途：

- Soft synth
- Pads
- Vocal chop
- Ambient textures
- Airy lead

---

# Follow Engine

Airの特徴的な部分。

生成したNoiseや倍音を単独で鳴らすのではなく、

> **元信号の動き・明るさ・発音に追従させる。**

---

## ENVELOPE FOLLOW

入力音量に追従。

```text
Input ↑
Air ↑

Input ↓
Air ↓
```

Noise Gate的に切るのではなく、滑らかに追従する。

---

## BRIGHTNESS FOLLOW

元音が明るい時だけAirを増やす。

Spectral centroidやHigh-band energyを利用。

目的：

> 暗い部分に無理に高域を足さない。

---

## TRANSIENT FOLLOW

アタック時だけAirを生成。

用途：

- Snare
- Clap
- Percussion
- Pluck
- Bass attack

---

## TONAL FOLLOW

生成TextureのBandpass中心や倍音構造を元音に追従。

Noiseを足している感じを減らし、

> **元音自身からAirが出ているように感じさせる。**

---

# Character Morph

SHINE / DUST / FIZZをMorphする。

```text
          SHINE
           /\
          /  \
         /    \
      DUST────FIZZ
```

中央付近では複数のTexture Generatorを混合。

例：

- Lead → SHINE寄り
- Snare → DUST寄り
- Bass → FIZZ寄り

BREATHは独立ノブまたは第4軸として扱う。

---

# Stereo Air

Originalの定位は保ったまま、生成AirだけをSide方向へ広げる。

```text
Original

MID   ███████████
SIDE  ███

Generated Air

MID   ███
SIDE  ██████████
```

低域・本体はセンターを維持しつつ、

> **周囲だけが光る**

状態を作る。

---

# Motion

Air Texture自体を動かす。

候補：

- Random pan
- Micro delay
- All-pass decorrelation
- Stereo flutter
- LFO width
- Particle density modulation

過度なHaas delayではなく、Mono compatibilityを維持する方向。

---

# Harshness Control

生成高域が刺さらないよう、2〜12 kHz付近を監視。

Airを増やした結果Peakが出すぎた場合、

- Generated Air gain ↓
- Dynamic notch ↑
- Fizz amount ↓
- Stereo spread ↓ optionally

とする。

---

# Main Controls

| Control | Role |
|---|---|
| AIR | 全体のTexture量 |
| SHINE | Harmonic Air |
| DUST | Noise Texture |
| FIZZ | Aggressive High-end |
| BREATH | Smooth Air Noise |
| FOLLOW | 入力追従量 |
| WIDTH | Air成分のStereo幅 |
| MIX | Dry / Wet |

---

# Character Modes

## Gloss

- Shine中心
- Clean harmonics
- Modern Lead向け

## Dust

- Envelope-following Noise
- Percussion / Texture向け

## Fizz

- Digital distortion
- Bass Music / Hyperpop向け

## Breath

- Smooth filtered Noise
- Pad / Vocal / Soft Synth向け

---

# Design Philosophy

AirはHigh ShelfやExciterではなく、

> **Signal-driven Texture Generator。**

ユーザーは高域を「上げる」のではなく、

- 光らせる
- 粒子を足す
- ザラつかせる
- 息を与える
- 周囲に広げる

という感覚で音の表面をデザインする。
