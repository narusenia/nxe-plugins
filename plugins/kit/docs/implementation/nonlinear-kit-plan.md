# Nonlinear Kit 計画 — Fold、Fever、Buckle

> 2026-11-25 に計画。ラインナップと順番の理由は
> [`series-plan.md`](../../../../docs/implementation/series-plan.md) の「Kit」。
> ここは単位と、出荷後に変えられないものを持つ。
> 3 本とも Design の Devour から切り出す Kit で、Devour の非線形部を先に
> 実地で確かめるために出す。

## 決めたこと

1. **1 本ずつ別のプラグイン**（`series-plan.md`）。つまみは 3 つ、3 本とも
   最後が `Mix`
2. **DSP は `nonlinear-core` に 3 つまとめて置く。** Devour が来たらそこへ
   そのまま移す。プラグインのクレート（`fold` など）は宣言と窓だけを持つ。
   `nxe-audio` には移さない。2 本目の呼び手が来たら移す、という
   `architecture.md` の規則のとおり
3. **窓は Kit の型（`nxe_egui::kit`）。** 左の縦帯は `DESIGN KIT`、差し色は
   Design の色 `Palette::DESIGN_KIT`（アシッドグリーン `#D7FF3A`、キャンバスの
   Kit 7）。図は 3 本とも**伝達曲線**（入力 → 出力）で、そのとき入力が届いて
   いる範囲を塗る。右上にプリセットの帯（`v2-plan.md` の決定 17）
4. **窓とプリセットの共通部分は `kit-ui` に置く。** バインド（`peek-ui` と同じ
   数行）、付属プリセットの表を引数に取るプリセットの帯、窓の骨組み
5. **非線形は 4 倍オーバーサンプリングの中で回す**（`nxe_audio::Oversampler`、
   切り替えなし）。**Mix もオーバーサンプリングの中で混ぜる。** 乾いた音も
   同じフィルタを通るので、IIR のハーフバンドの位相差で Mix 50 % が櫛形に
   ならない。代わりに Mix 0 % でもビット一致にはならない
6. **出力の直流は 10 Hz の 1 次ハイパスで落とす。** 非対称にすると、音量に
   応じて直流が動くので、固定の引き算では消えない
7. 3 本の中身:
   - **Fold** — 正弦の折り畳み `sin(π/2·(d·x + s)) − sin(π/2·s)`。
     `Fold`（d、1〜8 倍）、`Symmetry`（s、−100〜+100 %）、`Mix`。
     出力は `2/π` 倍して、d = 1・s = 0 の小さい信号で利得 1 にする
   - **Fever** — 入力の電力で**熱**が溜まり、熱がドライブと偏りを上げ、
     熱いほど揺らぐ飽和。`Fever`（熱のたまりやすさ）、`Recovery`（冷める時定数、
     0.2〜10 s）、`Mix`。揺らぎは決定的な疑似乱数を 3 Hz で丸めたもので、
     同じ入力なら同じ出力になる
   - **Buckle** — しきい値を越えた瞬間に波形が**裏返る**（座屈）。下がっても
     しきい値の半分（−6 dB）を割るまで戻らない、ヒステリシスつき。
     `Threshold`（−36〜0 dBFS）、`Snap`（裏返りの深さ、0〜100 %）、`Mix`。
     切り替わりは 0.5 ms でならす（不連続のまま出すとエイリアスが増える）

## 出荷後に変えられないもの

| | Fold | Fever | Buckle |
|---|---|---|---|
| 名前（`truce.toml`） | `Fold` | `Fever` | `Buckle` |
| `bundle_id` | `fold` | `fever` | `buckle` |
| AU `fourcc` | `Fold` | `Fevr` | `Bckl` |
| パラメーターのフィールド | `fold`、`symmetry`、`mix` | `fever`、`recovery`、`mix` | `threshold`、`snap`、`mix` |
| 版 | 1.0.0 | 1.0.0 | 1.0.0 |

リリースは `series.toml` の `[kit]`、タグ `kit-<YYYY.MM>[-beta.N]`。

## 単位

| ID | 単位 | 依存 | 状態 |
|---|---|---|---|
| KIT-1 | `Palette::DESIGN_KIT`、`kit-ui`、`nonlinear-core` の Fold、プラグイン Fold | — | 🟡 コードは済み。Windows の基準画像と耳が残り |
| KIT-2 | Fever | KIT-1 | 🟡 同上 |
| KIT-3 | Buckle | KIT-1 | 🟡 同上 |
| KIT-4 | 最初の beta（`kit-<YYYY.MM>-beta.N`） | KIT-1〜3 | ⬜ |

付属プリセットの中身と既定値は、ほかと同じく**耳で決める仕事として残る**。
ここに置いた値は最初の案。
