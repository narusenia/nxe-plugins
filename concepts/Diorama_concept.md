# Diorama Concept

## Overview

ボーカルの「残響量」ではなく、**前後位置・距離感そのものを操作する**ためのVocal Mix Processor。

> **Reverbを足すのではなく、声の“距離”を動かす。**

通常のReverbではWet量やDecayを操作することが中心だが、実際に人間が「近い / 遠い」と感じる要因は複数ある。

主な要素：

- Direct / Reflection 比
- Presence
- High-frequency loss
- Early Reflection
- Pre-delay
- Stereo width
- Transient clarity
- Low-mid diffusion

これらを統合し、1つの **DEPTH** マクロで自然に連動させる。

## Core Signal Flow

```text
Input
  ↓
Direct Analysis
  ↓
Presence / Transient Control
  ↓
Early Reflection Generator
  ↓
Distance-dependent EQ / Air Loss
  ↓
Stereo Depth Processing
  ↓
Focus / Presence Lock
  ↓
Mix
```

## Main Concept: DEPTH

```text
CLOSE  ←────────────→  FAR
```

DEPTHを動かすと、単にWet量だけでなく複数のパラメータが連動する。

### CLOSE

- Dry比率 ↑
- 2〜5kHz Presence ↑
- High-frequency detail ↑
- Early Reflection ↓
- Stereo Width ↓ slightly
- Transient clarity ↑
- Room contribution ↓

狙い：

- 囁き
- Intimate vocal
- Ballad
- センターに張り付くLead Vocal

### FAR

- Direct感 ↓
- Early Reflection ↑
- High-frequency damping ↑
- Presence ↓
- Diffuse Side成分 ↑
- Transient smoothing ↑
- Low-mid room contribution ↑

狙い：

- Backing vocal
- Ambient vocal
- Intro / Breakdown
- 遠景的な声

## Distance Components

### 1. DIRECT

声そのものの近さ。

担当する処理：

- Presence
- Transient
- Dry level
- Short compression
- Consonant clarity

### 2. ROOM

壁や空間との関係。長いReverbではなく、主に **Early Reflection** を扱う。

目安：`10〜120 ms`

目的：

- 空間の存在を感じさせる
- 歌詞の明瞭さを保つ
- Reverb Tailに頼らず距離感を作る

### 3. AIR LOSS

遠距離で発生する高域損失を模倣する。

- Dynamic High Shelf
- Reflection側だけHF Damping
- Transient時の高域保持
- DirectとDiffuseの異なる高域減衰

狙いは、遠くなっても「こもっただけ」に聞こえないこと。

## Distance-dependent Width

```text
DEPTH ↑

Direct Width      ↓ slightly
Reflection Width  ↑
Diffuse Side      ↑
```

結果として、**声本体はセンターに残るのに、遠く感じる**状態を作る。

## FOCUS

DEPTHとは独立した明瞭度コントロール。

FOCUSを上げると、

- Vocal formant周辺を保持
- ReflectionのMidを整理
- Early Reflection / Reverbを軽くDuck
- ConsonantだけDirect成分を残す
- Low-mid maskingを減少

する。

## PRESENCE LOCK

DEPTHを動かした際の明瞭度変化を監視し、必要に応じて自動補正する。

```text
DEPTH ↑
Presence naturally ↓

Presence Lock:
→ intelligibility補正
→ 2〜5kHzを必要量だけ保持
→ transient detailを部分的に復元
```

目的は、**距離は変わるが、声の存在感は必要以上に失わない**こと。

## MOTION

距離を固定ではなく、時間的に動かす。

Modulation source例：

- LFO
- Envelope Follower
- Sidechain
- MIDI / Automation

用途例：

```text
Verse      → Close
Pre Chorus → slightly Far
Chorus     → Wide / Larger Space
```

## Main Controls

| Control | Role |
|---|---|
| DEPTH | Close ↔ Far |
| DIRECT | 直接音の近さ |
| ROOM | Early Reflection量 |
| AIR | 距離による高域感 |
| WIDTH | 空間成分の広がり |
| FOCUS | 遠距離時の明瞭度 |
| MOTION | 距離の時間変化 |
| MIX | Dry / Wet |

## Character Modes

### Intimate
- Direct強め
- Reflection最小
- Presence高め
- Width狭め

### Studio
- 自然なEarly Reflection
- Balanced Presence
- 控えめなWidth

### Wide
- Reflection Width ↑
- Side成分 ↑
- Direct Center保持

### Distant
- HF Loss ↑
- Reflection ↑
- Transient softening ↑
- Presence ↓

## Design Philosophy

> **Reverbを操作することではなく、声の“位置”を操作すること。**

ユーザーが考えるべきなのはPre-delayやDecayではなく、**「この声をどこに置きたいか」**。

## Product Positioning

- **Vocal Saturator** — Body / Density / Presence / Texture
- **Sparkluer** — Air / Shine / Multiband Dynamics
- **Diorama** — Close / Far / Room / Focus / Width / Distance
- **Vocal Glue** — Cohesion / Shared dynamics / Stack / Lead anchoring
