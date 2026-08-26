# Vocal Glue Concept

## Overview

Lead / Double / Harmony / Adlibなど複数のボーカルトラックを、

> **同じ場所・同じ質感・同じエネルギーを持った“1つのボーカル群”にまとめる**

ためのVocal Bus Processor。

通常のBus Compressorだけでは音量はまとまっても、

- 別々に録った感じ
- 声質の差
- Stereoの分離感
- 発音タイミングのバラつき
- Room感の違い

までは解消しにくい。

Vocal Glueでは複数の処理を統合し、**Cohesionそのものを操作する**。

## Core Signal Flow

```text
Lead ─────┐
Double ───┤
Harmony ──┤
Adlib ────┘
     ↓
Shared Dynamics
     ↓
Spectral Cohesion
     ↓
Common Saturation
     ↓
Stereo Cohesion
     ↓
Ambience Glue
     ↓
Output
```

## Main Concept: GLUE

GLUEを上げると、

```text
Shared Compression ↑
Spectral Cohesion ↑
Common Saturation ↑
Stereo Cohesion ↑
Ambience Glue ↑
```

を連動させる。

狙いは、**「別々の声」から「1つの声群」に変化する**こと。

## 1. Shared Dynamics

瞬間Peakだけでなく **Phrase単位のEnvelope** を見る。

目的：

- Vocal Stack全体の呼吸を揃える
- 小さいHarmonyだけ置いていかれない
- LeadのDynamicsを壊しすぎない
- Groupとして一体感を出す

### BIND

BINDを上げると、

- Group compression ↑
- Slow envelope linking ↑
- Level deviation ↓
- Phrase cohesion ↑

する。

## 2. Spectral Cohesion

Lead / Double / Harmony間の音色差を整える。

よくある問題：

- Leadだけ明るい
- Doubleだけ鼻声
- HarmonyだけLow-midが多い
- Adlibだけ刺さる

Bus全体のSpectrumを解析し、極端に飛び出す帯域を動的に整える。

目的はResonance Suppressionではなく、**同じ“色”の中に収めること**。

処理例：

- Dynamic tilt matching
- Low-mid excess reduction
- High-mid outlier smoothing
- Average spectral centroidへの軽い収束

## 3. Common Saturation

全ボーカルを同じ非線形処理に通すことで、**同じ機材を通したような共通質感**を作る。

処理イメージ：

- Soft clipping
- Tape-like compression
- Even harmonics
- High-frequency smoothing
- 微量のintermodulation character

### COLOR

COLORを上げると、

- Harmonic consistency ↑
- Midrange density ↑
- High-frequency roughness ↓ slightly
- Vocal stack coloration ↑

する。

## 4. Stereo Cohesion

帯域・時間軸に応じてWidthを制御する。

```text
Low-mid → Slightly Center
High band → Wider
Onset → Center
Sustain → Wide
```

## ATTACK CENTERING

発音の瞬間だけSide成分を少し狭める。

```text
"Tonight"

T       → Center
onight  → Wide
```

効果：

- 子音が揃って聞こえる
- 発音がまとまる
- 広がりは失わない
- DoubleがLeadから剥がれにくい

## 5. Ambience Glue

最後に非常に薄い共通空間を追加する。

- Short Early Reflection
- Micro room
- Short diffuse tail

Wet量の目安：`0.5〜3%`

目的は、**全員が同じ場所に存在している感覚を作ること**。

## LEAD ANCHOR

Lead Vocalを基準にして、Double / Harmony / Adlibを周囲に配置する。

入力方法：

- Sidechain Lead Input
- Dedicated Lead Input
- 内部Route

Leadが発音した際に、

- Harmony Presence ↓ slightly
- Double Midrange ↓ slightly
- Side Vocal Level ↓ slightly
- Lead周辺のMaskingを除去
- Stereo imageのCenterをLeadに固定

する。

### ANCHOR Macro

```text
0%   → 全員対等
100% → Leadを絶対的な中心として配置
```

## STACK

大量のDouble / Harmonyを一枚のVocal StackとしてまとめるMacro。

STACKを上げると、

- Double Width ↑
- Harmony Density ↑
- Timing差を軽くSmooth
- Common Saturation ↑
- Ambience Glue ↑
- Spectral cohesion ↑

する。

用途：

- EDM Vocal
- Hyperpop
- Anime / J-Pop chorus
- Choir-like vocal stacks
- Large backing vocals

## Timing Cohesion

ピッチ補正ではなく、**微小な発音タイミング差**を扱う。

狙いは完全同期ではなく、**バラつきを残しながら、まとまりだけ増やす**こと。

処理候補：

- Onset detection
- Micro delay adjustment
- Attack-only alignment
- Sustained vowelはそのまま保持

## AIR Cohesion

複数のボーカルで高域感がバラバラな場合、Air帯域だけを共通化する。

処理例：

- Shared high-shelf tendency
- Sibilance-aware balancing
- Air saturation
- Common HF smoothing

## Main Controls

| Control | Role |
|---|---|
| GLUE | 全体のCohesion |
| BIND | Shared Dynamics |
| COLOR | 共通Saturation / Tone |
| STACK | Vocal Stackの一体化 |
| WIDTH | Stereo Cohesion |
| ANCHOR | Lead優先度 |
| AIR | High-frequency cohesion |
| ROOM | 共通Ambience |
| MIX | Dry / Wet |

## Advanced Sections

### Dynamics
- Compression
- Phrase linking
- Attack
- Release
- Envelope smoothing

### Spectral
- Cohesion amount
- Low-mid control
- Presence matching
- Air matching

### Stereo
- Width by band
- Attack centering
- Lead centering
- Side sustain

### Ambience
- Early Reflection
- Room size
- Width
- Damping
- Ducking

## Character Modes

### Tight
- Strong Attack Centering
- Narrower Low-mid
- Fast Dynamics
- Small Room

### Wide
- High-band Width ↑
- Stack ↑
- Ambience ↑
- Lead Center保持

### Smooth
- Spectral Cohesion ↑
- Soft Saturation
- Slow Dynamics
- Air smoothing

### Massive
- Stack ↑
- Density ↑
- Width ↑
- Common Saturation ↑

## Design Philosophy

> **コンプレッションすることではなく、複数の声を“同じ作品の1つの声”にすること。**

ユーザーが考えるべきなのはRatioやThresholdだけではなく、**「このボーカルたちは、どれだけ一体化して聞こえてほしいか」**。

## Product Positioning

- **Vocal Saturator** — Body / Density / Presence / Texture
- **Sparkluer** — Air / Shine / Multiband Dynamics
- **Vocal Depth** — Close / Far / Room / Distance / Focus
- **Vocal Glue** — Shared dynamics / Common tone / Stereo cohesion / Stack / Lead anchoring
