# Particle Kit 計画 — Glint、Pollen

> 2026-11-25 に計画。ラインナップは
> [`series-plan.md`](../../../../docs/implementation/series-plan.md) の表
> （Effloresce から Glint と Pollen を切り出す）。
> ここは単位と、出荷後に変えられないものを持つ。
> 元の案は [`concepts/series-ideas.md`](../../../../concepts/series-ideas.md) の
> 小型プラグイン案（Glint「トランジェントから結晶的な高域生成」、Pollen「音程を
> 維持した微細粒子拡散」）。

## 決めたこと

1. **Colour の Kit として出す。** 窓は Kit の型で、差し色は Dew、Halo と同じ
   `Palette::COLOUR_KIT`（アクア）。最後のつまみは `Wet`（フィールドは `mix`）。
   乾いた音を減らさず足すだけなのは Dew、Halo と同じ
2. **DSP は新しい `particle-core` に置く。** Effloresce が来たらそこから使う。
   `resonance-core` に入れないのは、Effloresce の中身（粒、正弦波の再合成）が
   共鳴器ではないから
3. **アタック検出を `nxe-audio::onset` に移した。** Dew のものを、2 本目の
   呼び手（Glint）が来たので中身を変えずに移した（`architecture.md` の規則）
4. **Glint の中身**（2026-11-25、ユーザーの選択は「アタックで高い倍音の束を
   鳴らす」、つまみは Sensitivity・Height・Decay・Wet）:
   - アタックごとに 5 つの火花を飛ばす。火花は柔らかく立ち上がる短い正弦で、
     指数で減衰する。同時に鳴るのは 16 まで
   - 高さは `Height`（3〜12 kHz）。アタックごとに束全体を ±0.3 オクターブ動かす
   - 入力の音程が取れていれば、火花はその倍音に乗る。取れていなければ鐘の比率
     （1、1.19、1.47、1.66、2）で鳴り、ガラスを叩いたような音になる。
     **鳴っている間も倍音へ寄せ直す**（Dew の Follow と同じ理由。アタックの時点
     では音程がまだ取れていない）
   - `Decay` は T60 で 30 ms〜1.2 s。高い火花ほど少し短い
   - 火花の大きさは、鳴り始めたときの入力の直近のピーク
   - 図は火花を、左右がパン、高さが音程の位置に十字の光として描く。光の長さが
     大きさで、鳴った直後に一瞬長く光る
5. **Pollen の方向**（2026-11-25、ユーザーの選択）: 入力を 20〜80 ms の粒に
   切り、音程は変えずに少し遅らせ、左右へばらまく。中身は Glint の後に決める

## 出荷後に変えられないもの

| | Glint |
|---|---|
| 名前（`truce.toml`） | `Glint` |
| `bundle_id` | `glint` |
| AU `fourcc` | `Glnk` |
| パラメーターのフィールド | `sensitivity`、`height`、`decay`、`mix` |
| 版 | 1.0.0 |

リリースは `series.toml` の `[kit]`、タグ `kit-<YYYY.MM>[-beta.N]`。

付属プリセット（Default、Frost、Chime、Shard、Spray）の中身は、耳で決める仕事
として残る。

## 単位

| ID | 単位 | 依存 | 状態 |
|---|---|---|---|
| PRT-1 | `nxe-audio::onset` への移動、`particle-core` の Glint、プラグイン Glint | — | 🟡 コードと基準画像（macOS / Windows）は済み。耳が残り |
| PRT-2 | Pollen | PRT-1 | ⬜ 中身を決めるところから |
