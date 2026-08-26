# Impact / Drop Enhancer Concept

## Overview

EDM / Bass MusicのDrop、Kick、Snare、Impact、Bus処理向けのEnergy Processor。

コンセプトは、

> **音量を上げるのではなく、「来た！」と感じる瞬間を作る。**

人間がDropやImpactを大きく感じる要因は、Peak Levelだけではない。

主な要素：

- Transient
- Low-frequency punch
- High-frequency crack
- Stereo contrast
- Short ambience
- Pre / Post contrast
- Saturation
- Temporal expansion

これらを統合し、瞬間的なサイズ感を作る。

---

# Core Signal Flow

```text
Input
  ↓
Transient / Event Detector
  ↓
Pre-Dip Generator
  ↓
Punch / Boom / Crack Processing
  ↓
Short Ambience / Early Reflection
  ↓
Stereo Expansion
  ↓
Saturation / Clipping
  ↓
Envelope-based Recovery
  ↓
Output
```

---

# IMPACT

メインMacro。

IMPACTを上げると、

```text
Transient ↑
Low Punch ↑
Short Room ↑
Stereo Width ↑
High-frequency Burst ↑
Saturation ↑
Pre-hit Contrast ↑
```

を連動させる。

単なるTransient Shaperではなく、

> **「音が大きく感じる理由」をまとめて増幅する。**

---

# PUNCH

音の最初の20〜100 ms付近を中心に強化。

用途：

- Kick head
- Snare attack
- Drop onset
- Bass hit
- Impact FX

処理候補：

- Transient extraction
- Short parallel compression
- Low-mid transient emphasis
- Envelope shaping

---

# BOOM

Impact後の低域エネルギー。

主に50〜200 Hz付近。

目的：

- 物理的な重量
- Impact後の余韻
- Drop headの巨大さ

単純なLow Shelfではなく、Event envelopeに合わせて短時間だけ生成する。

---

# CRACK

主に1〜10 kHz。

目的：

- Attack edge
- Snare crack
- Click
- High-frequency burst
- Perceived loudness

処理候補：

- Transient exciter
- Parallel clipping
- High-passed distortion
- Short noise burst

---

# PRE-DIP

Impact直前のコントラストを作る。

Lookaheadを使い、Hitの10〜40 ms前だけ、

- Level ↓ slightly
- Low-mid ↓ slightly
- Stereo width ↓ slightly

とする。

その直後にImpact処理を入れることで、

> **後のHitを相対的に大きく感じさせる。**

Pre-Dip量は極端にならないよう自動制限する。

---

# SIZE

音量ではなく、イベントの物理的なサイズ感。

SIZEを上げると、

- Early Reflection duration ↑
- Low-frequency decay ↑
- Reflection width ↑
- Diffusion ↑
- HF damping ↑ slightly

する。

長いReverbではなく、10〜150 ms程度のEarly Reflectionを中心に使用。

目的：

> **大きな空間ではなく、大きな物体が鳴っている感覚。**

---

# DROP MODE

Bus用途向け。

Drop開始をTransient / Level / Sidechainから検出し、最初の100〜500 msだけ処理を強める。

```text
Energy

100% |\ 
     | \
     |  \______
     |
     +---------- time
```

一瞬だけ、

- Width ↑
- Sub punch ↑
- Saturation ↑
- Exciter ↑
- Room size ↑

とし、その後通常状態へ戻す。

---

# Event Detection

検出方法：

- Transient
- RMS jump
- Spectral flux
- Sidechain trigger
- MIDI / Automation

これによりKick単体だけでなく、Drop Bus全体にも使用可能。

---

# Stereo Contrast

Hitの瞬間だけStereo imageを一時的に変化させる。

例：

```text
Before Hit
→ Narrower

On Hit
→ Wider

Sustain
→ Return
```

これによりLevelを増やさなくてもImpact感を増やせる。

---

# Main Controls

| Control | Role |
|---|---|
| IMPACT | 全体のImpact量 |
| PUNCH | Attack energy |
| BOOM | Low-end impact |
| CRACK | High-frequency attack |
| SIZE | 物理的サイズ感 |
| WIDTH | Stereo contrast |
| PRE-DIP | Hit前のコントラスト |
| MIX | Dry / Wet |

---

# Character Modes

## Tight

- Short attack
- Minimal room
- Strong crack
- Kick / Snare向け

## Huge

- Boom / Size強め
- Wide reflections
- Drop / Impact FX向け

## Aggressive

- Saturation / clip強め
- Crack強め
- Bass Music向け

## Cinematic

- Longer early reflections
- Deep low-frequency tail
- Transition FX向け

---

# Design Philosophy

Impact / Drop Enhancerは、

> **Peakを大きくするプラグインではなく、瞬間的なエネルギー差をデザインするプラグイン。**

DropやHitの「大きさ」は、音量だけでなく時間・周波数・空間のコントラストから作る。
