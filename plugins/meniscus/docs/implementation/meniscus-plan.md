# Meniscus 計画 — Colour の最初のフラッグシップ

> 2026-11-26 に計画。ラインナップと順番の理由は
> [`series-plan.md`](../../../../docs/implementation/series-plan.md) の「順番」4。
> 元の案は [`concepts/series-ideas.md`](../../../../concepts/series-ideas.md) の
> Meniscus（表面張力を模した音響挙動。トランジェントで共振周波数を急変させ、
> 揺れながら安定させる）と、まとめた Liquefy。Kit の Dew はここから切り出した。

## 決めたこと

1. **入力を「水面」に通す。** 水面は帰還型のコムフィルターで、決めた音程
   （**Free**）か入力のピッチ（**Follow**、Dew と同じく `nxe_audio::pitch`）の
   倍音で鳴る。帰還の中に tanh とローパスを入れ、非線形で、鳴るほど暗くなる
2. **アタックで水面を打つ。** アタックは `nxe_audio::onset`（**Sensitivity**）。
   打たれた水面は音程が一気に下がり（**Splash** で最大 1 オクターブ）、
   ばねのように行き過ぎて揺れながら戻る。ばねは 2 次の減衰振動で、
   **Tension** が揺れの速さ（2〜18 Hz）、**Viscosity** が収まる早さ。
   Viscosity は帰還のローパスも暗くする（粘い液体ほど鈍く鳴る）
3. **Resonance** は帰還の量（最大 0.97）。入力の量は `√(1 − fb²)` で割って、
   帰還を上げても広帯域の入力の音量が変わらないようにする
4. **Drops は Dew そのもの**（`resonance_core::dew`）。同じアタックで、水面の
   音程の 2 オクターブ上に落ちる。Size は Viscosity、Splash は Splash を使う
5. 右チャンネルの水面は左より 7 セント上で、広がりを出す
6. **Wet は足す量**（Kit と同じ）。0 で出力は入力と一致し、遅れはない。
   **Output** は最後に掛ける
7. **窓は Colour 6 の型をここで作る**（`nxe_egui::colour`、880 × 430）。
   左に操作部の面（名前、つまみ 8 個を 3 列、プリセット）、右に音名の帯と
   図の面。面は角を 8 丸める（2026-11-26、14 から控えめに）。**地は無彩色（`Palette::COLOUR`）で、色は音名
   にだけ乗る**（`colour::note`。半音ごとに色相を 30° 回す）
   - 帯は水面が鳴っている音名と Hz、その音名を Prisma と同じ 12 音名の格子で。
     右に Tuning と Pitch
   - 図は水面を横から見たもの。直近 1.5 秒の揺れを、真ん中を今にして左右へ
     広げるので、打たれた所が真ん中で沈み、揺れが波紋のように外へ走る。
     水面の線と下の塗りは、鳴っている音名の色。水滴は上に Dew と同じ輪で出る

## 出荷後に変えられないもの

| | Meniscus |
|---|---|
| 名前（`truce.toml`） | `Meniscus` |
| `bundle_id` | `meniscus` |
| AU `fourcc` | `Mnsc` |
| パラメーターのフィールド | `sensitivity`、`tension`、`viscosity`、`splash`、`resonance`、`drops`、`mix`（表示は Wet）、`output`、`tuning`、`pitch` |
| 版 | 1.0.0 |

リリースは `series.toml` の `[colour]`。

## 単位

| ID | 単位 | 依存 | 状態 |
|---|---|---|---|
| MEN-1 | `resonance_core::meniscus`（水面、打ち、Drops） | RES-1 | ✅ 試験 9 本 |
| MEN-2 | `nxe_egui::colour`（Colour 6 の枠、`Palette::COLOUR`、音名の色、チップ） | — | ✅ |
| MEN-3 | Meniscus のプラグイン（宣言、窓、付属プリセット） | MEN-1, MEN-2 | 🟡 コードと macOS の基準画像は済み。Windows の基準画像と耳が残り |
| MEN-4 | 最初の beta（`colour-<YYYY.MM>-beta.N`） | MEN-3 | ⬜ |

付属プリセット（Default、Jelly、Bubble、Honey、Mercury）の値は仮。
