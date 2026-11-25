# Resonance Kit 計画 — Dew、Halo

> 2026-11-25 に計画。ラインナップと順番の理由は
> [`series-plan.md`](../../../../docs/implementation/series-plan.md) の「順番」4。
> ここは単位と、出荷後に変えられないものを持つ。
> 2 本とも Colour の Kit で、Dew は Meniscus から、Halo は Prisma から切り出す。
> 元の案は [`concepts/series-ideas.md`](../../../../concepts/series-ideas.md) の
> 小型プラグイン案（Dew「水滴状の倍音トランジェント付加」、Halo「原音の周囲に
> 調性的残響を生成」）。

## 決めたこと

1. **1 本ずつ別のプラグイン**、つまみは 3 つで最後が `Mix`（ほかの Kit と同じ）
2. **DSP は `resonance-core` に置く。** Meniscus と Prisma が来たらそこから
   使う。プラグインのクレートは宣言と窓だけを持つ
3. **差し色は Colour の色 `Palette::COLOUR_KIT`（アクア `#5CE1E6`）**
   （2026-11-25、ユーザーの選択）。左の縦帯は `COLOUR KIT`。窓は Kit の型
4. **リリースは `[kit]` シリーズに入れる。** Design の Kit と同じシリーズで、
   色で見分ける
5. **Dew の中身**:
   - 入力のアタック（2 ms と 60 ms の電力の平均の比が 6 dB を越えたとき。
     次のアタックまで 50 ms 空ける）ごとに、**水滴**を 1〜6 滴落とす。
     水滴は減衰する正弦で、音程は少し下（落ち着く先の 1/1.6）から上に滑って
     落ち着く。泡が閉じるときの鳴り方
   - `Size`（0〜100 %）は滴の大きさ。大きいほど低く（4 kHz から 3 オクターブ下
     の 500 Hz まで）、長く鳴る（減衰の時定数 25〜150 ms）
   - `Splash`（0〜100 %）は 1 回のアタックで落ちる滴の数（1〜6）、時間の散らばり
     （最大 60 ms）、左右の広がり、音程のばらつき（±1〜6 半音）
   - `Mix` は**足す量**。乾いた音はいつもそのまま通り、Mix 0 % で入力と
     ビット一致になる。遅らせないのでレイテンシはない
   - **`Tuning` で音程の決め方を切り替える**（2026-11-25、ユーザーの選択）。
     `Free` は Size が決めた音程、`Follow` は入力のピッチの倍音のうち、そこに
     一番近いものに落ち着く。ピッチが取れない（明瞭さ 0.85 未満）ときは Free と
     同じ
   - 乱数は決定的なので、同じ入力なら同じ水滴が落ちる
   - 図は、鳴っている滴を**波紋**で描く。横は左右の位置、縦は音程（100 Hz〜
     8 kHz）、時間とともに広がり、減衰とともに消える
6. **ピッチ追従は `nxe-audio::pitch` に移した。** Parasite の YIN を、2 本目の
   呼び手（Dew の Follow）が来たので中身を変えずに移した（`architecture.md` の
   規則）

## 出荷後に変えられないもの

| | Dew |
|---|---|
| 名前（`truce.toml`） | `Dew` |
| `bundle_id` | `dew` |
| AU `fourcc` | `Dewk` |
| パラメーターのフィールド | `size`、`splash`、`mix`、`tuning` |
| 版 | 1.0.0 |

リリースは `series.toml` の `[kit]`、タグ `kit-<YYYY.MM>[-beta.N]`。

## 単位

| ID | 単位 | 依存 | 状態 |
|---|---|---|---|
| RES-1 | `nxe-audio::pitch` への移動、`Palette::COLOUR_KIT`、`resonance-core` の Dew、プラグイン Dew | — | 🟡 コードと macOS の基準画像は済み。Windows の基準画像と耳が残り |
| RES-2 | Halo | RES-1 | ⬜ |

付属プリセット（Default、Drip、Rain、Mist、Puddle）の中身は、耳で決める仕事と
して残る。
