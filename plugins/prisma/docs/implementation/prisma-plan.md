# Prisma 計画 — Colour の 2 本目のフラッグシップ

> 2026-11-26 に計画。ラインナップと順番の理由は
> [`series-plan.md`](../../../../docs/implementation/series-plan.md) の「順番」4。
> 元の案は [`concepts/series-ideas.md`](../../../../concepts/series-ideas.md) の
> Prisma（MIDI 制御のポリフォニック共振、非整数倍音、共振器間結合、Decay）と、
> まとめた Mycelia（結合共振器）、Aether のフリーズ。Kit の Halo はここから
> 切り出した。

## 決めたこと

1. **入力を、弾いた音に分ける。** 最大 8 音、1 音に倍音 1〜4 の共振器を 4 本。
   共振器は Halo のもの（`resonance_core::halo` の複素一極、捕まえる幅、
   入力の最近のピークに対する上限。どれも **Sensitivity** で動く）
2. **音は MIDI か和音か**（**Source**）。
   - **MIDI** — 押さえている音が鳴る。離した音は入力を受けなくなり、
     Decay で鳴り終わる。9 音目は、空き → 離した中で古いもの → いちばん古い
     ものの順に場所を取る。ノートはサンプル単位で当てる
   - **Chord** — Halo と同じ和音（Fifth / Maj / Min）を、決めた Root か入力の
     ピッチ（**Tuning** の Follow）の上に積む。MIDI がなくても使える
3. **Decay** は T60 で 0.3〜12 秒。**Inharmonic** は倍音を `k·√(1 + B·k²)` で
   上へ広げる（硬い弦や鐘、B は最大 0.03、4 倍音が約 1.28 倍ずれる）。
   **Tone** は上の倍音を暗くから明るくへ傾ける（Halo と同じ）、**Shimmer** は
   音ごとにゆっくり揺らす
4. **Coupling は共振器どうしが鳴らし合う量**（Mycelia）。どの共振器も、
   自分以外の全部の出力を少し聞く。同じ高さに重なる倍音（A2 の 2 倍音と A3）
   が互いを支え、入力が止まったあとも鳴りが残る。入力のピークに対する上限は
   Coupling に応じてゆっくり下げ、鳴りが自分の Decay を越えて残れるようにする
5. **Freeze** はその時の鳴りを止めて保つ。減衰はほぼ 0、新しい入力は受けない
6. **MIDI を受けるエフェクト**（`truce.toml` の `midi_input = true`）。
   macOS の AU は `aufx` でなく `aumf`（music effect）になる
7. **Wet は足す量**、**Output** は最後に掛ける。遅れはない
8. **窓は Meniscus が作った Colour 6 の型**（`nxe_egui::colour`）
   - 帯は左にコード名、右に 12 音名の格子（`colour::classes`。2026-11-26、
     丸いチップが AI っぽいとのことで格子に替えた）。鳴っている音名だけ
     その色で書き、下にオクターブの数だけ棒を引く
   - コード名は鳴っている音から付ける（`colour::chord_name`、Chord でも
     MIDI でも同じ）。型（maj、m、5、7、maj7、sus、dim、aug、6、9 など）に
     ぴったり合うものを、いちばん低い音を根音として先に試し、別の根音なら
     `C/E` のように書く。何も鳴っていなければ `N.C.`、名前の付かない
     重なりは「N notes」
   - **図はプリズム**（2026-11-26、ユーザーの依頼「プリズムみたいな表現。
     Swiss デザインは崩さない」）。左から入力が白い 1 本の光で入り、三角形を
     通って、鳴っている倍音ごとに音名の色の光線に分かれ、右のスクリーン
     （音程の目盛り）の、その高さに当たる。高い倍音ほど大きく曲がる。
     当たった所に鳴っている長さの棒が伸びる。線と面だけで描き、グラデーション
     は使わない。止まっている時も Chord なら鳴るはずの光線を薄く出す
   - 下の 1 行に Source、Chord、Tuning、Root、Freeze

## 出荷後に変えられないもの

| | Prisma |
|---|---|
| 名前（`truce.toml`） | `Prisma` |
| `bundle_id` | `prisma` |
| AU `fourcc` | `Prsm`（Parasite は `Prst`） |
| AU の種類 | `aumf`（MIDI を受けるため） |
| パラメーターのフィールド | `sensitivity`、`decay`、`inharmonic`、`coupling`、`tone`、`shimmer`、`mix`（表示は Wet）、`output`、`source`、`chord`、`tuning`、`root`、`freeze` |
| 版 | 1.0.0 |

リリースは `series.toml` の `[colour]`。

## 単位

| ID | 単位 | 依存 | 状態 |
|---|---|---|---|
| PRI-1 | `resonance_core::prisma`（音、倍音、Coupling、Freeze） | RES-2 | ✅ 試験 10 本 |
| PRI-2 | Prisma のプラグイン（宣言、MIDI、窓、付属プリセット） | PRI-1, MEN-2 | 🟡 コードと macOS の基準画像は済み。Windows の基準画像と、DAW で MIDI を送って耳で確かめることが残り |

付属プリセット（Default、Glass、Bell Choir、Mycelium、Halo）の値は仮。
