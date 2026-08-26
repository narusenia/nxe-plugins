# Sparkleur — DSP 仕様

> 最終更新: 2026-08-26

要件は [`../requirements/REQ-SPK.md`](../requirements/REQ-SPK.md)。守るべき規約は
[`../../../../.agents/rules/rust.md`](../../../../.agents/rules/rust.md)。
共有部品は `crates/nxe-audio`（`REQ-SPK-015`）。

**この文書と実装が食い違ったら実装が正。** 同じ変更でこの文書を直す。

## 信号の流れ

```text
Input (L,R)
  │
  ├──────────────────────────────── dry ────────────────────────────┐
  │                                                                 │
  ├─ Crossover ─┬─ band 1 ─ Dynamics ─ × gain₁ ──┐                  │
  │  LR4 + AP   ├─ band 2 ─ Dynamics ─ × gain₂ ──┤                  │
  │  (5 帯域)   ├─ band 3 ─ Dynamics ─ × gain₃ ──┼─ Σ ─── wet ──────┤
  │             ├─ band 4 ─ Dynamics ─ × gain₄ ──┤                  │
  │             │            ↑ De-Harsh          │                  │
  │             └─ band 5 ─ Dynamics ─ × gain₅ ──┤                  │
  │                   │                          │                  │
  │                   └─ Sparkle ────────────────┘                  │
  │                      (4x → shaper → HPF → × snap × AIR)         │
  │                                                                 │
  └─ 検波（モノ和、帯域ごと 1 本）→ 上下のゲイン計算                  │
                                                                    │
                                          out = dry + MIX·wet ──────┴─ × OUTPUT
```

**原音は分割前で分岐する**（`REQ-SPK-001`）。`MIX` = 0 が加算 1 回で入力
そのものになるのはこれだけが理由。

## クロスオーバー

Linkwitz-Riley 4 次 = Butterworth（`Q = 1/√2`）を 2 段。`nxe_audio::biquad` の
`lowpass` / `highpass` をそのまま重ねる。

境界 4 点（`FOCUS` = 0 のとき）:

| 境界 | Hz |
|---|---|
| 1 / 2 | 120 |
| 2 / 3 | 400 |
| 3 / 4 | 1 500 |
| 4 / 5 | 6 000 |

`FOCUS`（−1〜+1）で全部を `2^(focus · 1.5)` 倍する（±1.5 オクターブ）。
**4 点とも** `min(edge · shift, host_rate · 0.45)` で切る（実装は 4 点に
同じ式を当てている — 上端以外は 44.1 kHz 以上では絶対に当たらないので、
1 点だけ特別扱いする理由が無い）。

**天井が順序を崩しても平坦性は落ちない。** `LP(f) + HP(f) = AP(f)` は
コーナーの値に依らないので、境界が 2 つ同じ周波数に乗ると間の帯域が空に
なるだけで、和は相変わらずオールパス。順序を守らせるのは画面に出す数字の
都合であって、音の都合ではない。

### オールパス補正

木構造で分けると、**先に分けた帯域は後段のクロスオーバーの位相回転を受けない**
ので、和にリプルが出る。補正は「既に分けた帯域を、残りのクロスオーバーと同じ
オールパスに通す」。

```text
split(120):   low₁, rest₁
split(400):   low₂, rest₂  ← rest₁ から
split(1500):  low₃, rest₃  ← rest₂ から
split(6000):  low₄, band5  ← rest₃ から

band1 = low₁ を AP(400) · AP(1500) · AP(6000) に通したもの
band2 = low₂ を          AP(1500) · AP(6000) に通したもの
band3 = low₃ を                     AP(6000) に通したもの
band4 = low₄
band5 = band5
```

`AP(f)` は LR4 のクロスオーバーと同じ位相を持つ 2 次オールパスを 2 段
（`LP(f) + HP(f)` の和と等価）。**素直に `LP + HP` を計算して足す実装でよい** —
biquad は安いので、専用のオールパスを書くより読める。

**検証**: 全帯域 unity で和を取り、20 Hz〜20 kHz の振幅が **±0.1 dB 以内**
（`REQ-SPK-001`）。これが通らない限り他のテストは意味を持たない。

**通った**（`SPK-2`）。44.1 / 48 / 96 / 192 kHz と `FOCUS` −1〜+1 の全部で
±0.1 dB 以内。

**帯域の裾は 24 dB/oct とは限らない。** 木構造なので、ある帯域は自分の
境界だけでなく**上流の全部のハイパス**も通っている。band 5 の 6 kHz より
1 オクターブ下は `HP(1500)` がまだ効いていて **30 dB/oct** で落ちる。
24 dB/oct が素で見えるのは境界が 1 つしか効かないところだけ（band 1 の
120 Hz より上、band 2 の 120 Hz より下）。**検波や UI がここを勘違いすると
「フィルタの次数が違う」と読める。**

**コスト**: 1 チャネルあたり biquad 40 個（分割 4 × 4 + 補正 6 × 4）。
補正を専用の 2 次オールパス（1 個 = biquad 1）にすると 22 個まで落ちる。
**`SPK-17` で予算が厳しければ最初に触るのはここ** — 同じフィルタなので
音は変わらない。

## 検波

帯域ごとに 1 本、**モノ和**を帯域通過した後のパワー（`REQ-SPK-004`）。
`nxe_audio::guard` の `Follower` と同じ形。

```text
e_i ← 二乗して attack_i / release_i の 1 次追従
L_i = 10·log10(max(e_i, 1e-10))        (dBFS 相当のパワー)
```

**帯域通過は要らない。** クロスオーバーが既に分けているので、その帯域の信号を
そのまま二乗すればよい（Velour の Guard は分割していない信号から検出する必要が
あったので BPF を持っていた — ここは構造が違う）。

## 時定数（`SPEED`）

```text
f_ref_i   = √(low_i · high_i)                 最下段の low は 30 Hz として扱う
floor_a_i = 2 / f_ref_i                       2 周期
floor_r_i = 8 / f_ref_i                       8 周期
attack_i  = max(A(speed), floor_a_i)
release_i = max(R(speed), floor_r_i)

A(speed) = 1 ms  · (50/1)^(1−speed)           速い側 1 ms / 遅い側 50 ms
R(speed) = 20 ms · (400/20)^(1−speed)         速い側 20 ms / 遅い側 400 ms
```

`speed` は 0〜1 で、**大きいほど速い**。`CHARACTER` が中心をずらす
（POLISH で遅い側、CRUSH で速い側）。

導出された床の実際の値:

| 帯域 | `f_ref` | attack の床 | release の床 |
|---|---|---|---|
| 1（30〜120） | 60 Hz | 33 ms | 133 ms |
| 2（120〜400） | 219 Hz | 9.1 ms | 37 ms |
| 3（400〜1.5k） | 775 Hz | 2.6 ms | 10 ms |
| 4（1.5k〜6k） | 3.0 kHz | 0.7 ms | 2.7 ms |
| 5（6k〜20k） | 11 kHz | 0.2 ms | 0.7 ms |

**床が効くのは下 2 帯域だけ**で、上の 3 帯域では `SPEED` が支配する。
これが「LOW だけ速くできない」の実装（`REQ-SPK-005`）。

## ゲイン計算

帯域ごと、検波値 `L`（dB）から。

```text
down = min(0, (T_down − L) · (1 − 1/R_down))        しきい値超過を圧縮
up   = max(0, (T_up   − L) · (1 − 1/R_up))          しきい値以下を持ち上げ
up  ← min(up, CEILING_i)                            上限（帯域ごと）
up  ← up · taper(L)                                 床
gain_db_i = (down · w_down_i + up · w_up_i) · SPARK + GAIN_i
```

- `w_down_i` / `w_up_i` は per-band の**重み**（0〜1、`REQ-SPK-009`）。
  `SPARK` が倍率。
- `T_down` / `T_up` / `R_down` / `R_up` / ニー幅は `CHARACTER` が決める。
  しきい値は**入力レベルに対する絶対値**（仮 `T_down` = −18 dB、
  `T_up` = −36 dB）で、ここは耳で詰める。
- ニーは 2 次の soft knee。幅 `W`（`CHARACTER`、広い 12 dB ↔ 硬い 1 dB）。

### 床（taper）

```text
FLOOR_DB = −60          仮。Advanced で −90 まで開けられる
FADE_DB  = 12

taper(L) = clamp((L − (FLOOR_DB − FADE_DB)) / FADE_DB, 0, 1)
```

**これが無いと無音とノイズフロアとリリースの尻尾が持ち上がる**
（`REQ-SPK-003`）。OTT でベースに使ったときのポンピングの正体。
**Advanced で床を下げられる**ようにしてあるのは「VO-TT の代わり」を名乗る
ための逃げ道で、振り切ると OTT らしい呼吸が出る。

### Sub Protect

専用の処理ではない。`CEILING_i` の値違い（`REQ-SPK-008`）。

```text
CEILING_i = CHARACTER の上限 × (i == 1 ? (1 − sub_protect) : 1)
```

`sub_protect` = 1 で LOW の上げがちょうど 0 になる。

## CHARACTER

アンカー 3 点の表を補間（`REQ-SPK-006`）。Velour の `texture.rs` と同じ機構で、
**同じコードにはしない**（乗っている項目が違う）。

| | POLISH | GLOSS | CRUSH |
|---|---|---|---|
| `R_down` | 1.5 | 2.5 | 6.0 |
| `R_up` | 1.2 | 1.5 | 3.0 |
| `CEILING` (dB) | 6 | 9 | 15 |
| ニー幅 `W` (dB) | 12 | 6 | 1 |
| Sparkle `h` | 0.15 | 0.35 | 0.80 |
| Sparkle `β` | 0.50 | 0.30 | 0.10 |
| De-Harsh | 1.0 | 0.6 | 0.2 |
| Sub Protect | 0.0 | 0.4 | 1.0 |
| `SPEED` の中心 | 0.35 | 0.5 | 0.75 |
| レベルトリム (dB) | 仮 0 | 仮 0 | 仮 0 |

**レベルトリムは最初から置く。** Velour で `TEXTURE` を作ったとき、
カーブを変えるとレベルが −3.0 dB 動いて後から吸収する必要が出た。
**同じことが起きる場所**（比とニーを変えれば平均レベルは動く）なので、
最初から枠を用意して `SPK-17` で詰める。

## Sparkle

```text
band5 ─→ Oversampler(4x) ─→ shaper(β, h) ─→ ─→ HPF(6 kHz) ─→ × g ─→ 加算
                                                              ↑
 band5 のパワー ─┬─ fast follower (1 ms / 40 ms) ─┐            │
                └─ slow follower (100 ms) ────────┴─ 比 → snap ┘

snap  = clamp(10·log10(fast/slow) / SNAP_RANGE_DB, 0, 1)     SNAP_RANGE_DB = 仮 6
g     = AIR · ((1 − SNAP) + SNAP · snap)
```

- `SNAP` = 0 → `g = AIR`（完全に静的。Velour と同じ挙動）
- `SNAP` = 1 → `g = AIR · snap`（アタックのときだけ光る）
- **入力に上蓋**: `band5` の上端を `min(20 kHz, host_rate · 0.25)` で切る。
  Velour の `AIR_INPUT_CEILING` と同じ理由で、**無いとどんなドライブでも
  −44 dB で折り返す**。
- 生成後の HPF は「足すのは上だけ」を保証する。倍音は下にも降りてくるので、
  切らないと中域が濁る（Velour の各帯域の出力フィルタと同じ役割）。
- **`shaper` の正規化がそのまま効く。** `nxe_audio::shaper` は参照振幅での
  RMS ゲインを 1 に正規化してあるので、`CHARACTER` で `(β, h)` が動いても
  **足す量は動かない**。`REQ-SPK-010` の直交はこれに乗っている。

## De-Harsh

`nxe_audio::guard`（一般化後の `RelativeGuard`）を 1 インスタンス。

| | 値 |
|---|---|
| 検出帯 | 1.5〜5 kHz |
| 参照帯 | 300 Hz〜8 kHz |
| しきい値 | 仮 −8 dB（帯域/参照のパワー比） |
| 引く相手 | band 4（PRESENCE）の出力ゲイン |
| 上限 | 12 dB |
| 弾道 | attack 2 ms / release 60 ms |

**相対検出なので入力ゲインに依存しない** — 分子と分母が一緒に動く。
Velour の Harsh Guard で実測して固定してある性質（±12 dB で 0.2 dB 以内）。

**検出帯は分割後の band 4 ではなく、専用の帯域通過**で取る。理由: band 4 の
境界は `FOCUS` で動くが、「耳に痛い場所」は動かない。

## 耳で詰める定数

**機構が仕様、値は仮**（`SPK-17`）。

| 何 | 仮の値 | 聴きどころ |
|---|---|---|
| `T_down` / `T_up` | −18 / −36 dB | 普通のミックス素材でどちらも動くか |
| `CHARACTER` の表 | 上記 | POLISH と CRUSH が別の質感か。中間が使えるか |
| `CHARACTER` のレベルトリム | 0 / 0 / 0 | 軸を回してラウドネスが動かないか |
| `FLOOR_DB` | −60 dB | 無音で呼吸しないか。開放したとき OTT らしくなるか |
| `SNAP_RANGE_DB` | 6 dB | 子音とハットで光り、パッドで光らないか |
| De-Harsh のしきい値 | −8 dB | 普通の素材で**動かない**か |
| 境界 4 点 | 120 / 400 / 1.5k / 6k | 声とベースとシンセで足りるか。`FOCUS` の可動域は十分か |
| `SPEED` の範囲 | 1〜50 / 20〜400 ms | 速い側で低域が歪まないか |
| 既定値 33 個 | — | プリセットが無いので既定値が製品の顔 |

## 検証

| 何 | どう測るか |
|---|---|
| 分割の平坦性 | 全 unity で和を取り、対数スイープの振幅が ±0.1 dB 以内 |
| `MIX` = 0 | 出力が入力とビット一致 |
| 下げの比 | 一定正弦の振幅を変えて、しきい値超過分と出力の傾きの比 |
| 上げの上限 | 微小信号を入れてゲインが `CEILING` を超えないこと |
| 床 | 無音（+ ノイズ）でゲインが上がらないこと |
| 時定数の床 | `SPEED` 最速で LOW の attack を測り、導出値を下回らないこと |
| 低域の歪み | `SPEED` 最速、50 Hz 正弦で THD を測る |
| Sparkle の折り返し | 最悪ケースの入力で 4x の折り返しが −60 dB 以下 |
| De-Harsh の入力ゲイン非依存 | ±12 dB で動作量が 0.2 dB 以内 |
| レート非依存 | 44.1 / 48 / 96 kHz でレベル 0.5 dB / 倍音比 10% 以内 |
| ブロック非依存 | 1 / 32 / 512 / 2048 で出力一致 |
| hostile な値 | 全パラメータ・全サンプルに NaN / ±inf / ±1e9 を入れて非有限を出さない |
| CPU | criterion で 4x / 512 / 48 kHz が 533 µs 以内 |
