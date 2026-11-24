# Spectral Kit 計画 — Invert、Shear、Afterimage

> 2026-11-25 に計画。ラインナップと順番の理由は
> [`series-plan.md`](../../../../docs/implementation/series-plan.md) の「Kit」。
> 窓・プリセット・共通部分の決めごとは
> [`nonlinear-kit-plan.md`](nonlinear-kit-plan.md) と同じで、ここは違うところだけを持つ。
> Invert と Shear は Singularity、Afterimage は Dislocate から切り出す。

## 決めたこと

1. **STFT を `pumice-core` から `crates/nxe-spectral` に移す。** Spectral Kit が
   2 本目の呼び手なので、`architecture.md` の規則どおり共通クレートにする。
   `pumice-core` は `pub use nxe_spectral::{quality, stft}` で同じ道を残すので、
   Pumice のコードは変わらない
2. **DSP は `spectral-core` に 3 つまとめて置く。** Singularity と Dislocate が
   来たらそこへ移す
3. **変換は `Quality::Normal` に固定**（1 ビン約 23 Hz、48 kHz で 2048 点）。
   レイテンシは 1 ブロック（48 kHz で約 43 ms）で、Pumice と同じくホストに
   報告する。切り替えは出さない（つまみ 3 つの Kit に、レイテンシの変わる
   選択肢は置かない）
4. **Mix はスペクトルの上で混ぜる。** 乾いた音も同じ変換を通って戻るので、
   遅延補償なしで揃う
5. 3 本の中身:
   - **Invert** — `Low..=High` の中で、`f` を `Low + High − f` に移す。
     ほかの帯域は触らない。位相ボコーダーで各ビンの本当の周波数を読み、
     鏡に映した周波数で位相を回し続けるので、移した先の周波数は
     ビンに丸めない。`Low`、`High`（どちらも 20 Hz〜20 kHz、逆でもよい）、`Mix`
   - **Shear** — 周波数ごとに遅れを変える。対数の周波数軸に沿って、片方の端で 0、
     もう片方の端で `Length`。**対数軸上の位置の 2 乗**で遅らせる（遅れる端が
     扇のように開き、早い端はまとまる。直線だと定規のようだった、2026-11-25）。
     `Shear` が正なら高域が遅れ、負なら低域が遅れる。
     ビンを丸ごとフレーム単位で遅らせるので、重ね合わせで元の帯域がそのまま
     遅れて戻る（過去のスペクトルのリングを持つだけ）。`Shear`（−100〜+100 %）、
     `Length`（20〜1000 ms）、`Mix`
   - **Afterimage** — 各ビンが鳴った最大値を覚え、`Persistence` 秒で減らす
     （`Damping` が大きいほど高域が早く消える）。**今鳴っている分は足さない**:
     足すのは残像が今の音を上回る分だけなので、持続音はそのまま通る。
     残像は捉えた周波数で位相を回し続けて鳴らす。`Persistence`（0.2〜20 s）、
     `Damping`、`Mix`
6. **図**。Invert はパラメーターから描く（周波数ごとの移し先）。
   **Shear と Afterimage は今鳴っている音を描く**（2026-11-25、ユーザーの依頼）。
   エンジンがフレームごとにスペクトルを 96 の対数帯域にまとめ、Peek と同じ
   受け渡し（`nxe_dsp::Handoff`）で窓に渡す
   - Shear — 左に出力のスペクトログラム（`peek_ui::spectrogram::figure_frames`）、
     右に同じ周波数軸で周波数ごとの遅れを扇で描く。スペクトログラムは**2 秒、
     変換のフレーム 1 枚に 1 列**。音声側が直近 8 フレームを渡し、窓は届いた分を
     全部描く（30 Hz の窓で最新だけを読むと、数十 ms の傾きが見えなかった）
   - Afterimage — 今の入力のスペクトルを塗り、残像をアクセントの線で重ねる。
     音を止めると塗りが落ち、線だけが残って消えていく。その間に、少し前の入力の
     スペクトルを 12 本、古いほど薄くして引きずる（間隔は `Persistence` の
     12 分の 1、1/30〜0.25 秒）。残像の線の下も薄く塗る。今の音の塗りは灰色でなくアクセントを暗くした色（灰色はダサかった。2026-11-25、
     「もっと残像感」の依頼。音は変えていない）
   - **バイパス中は描かない。** ホストが `process` を呼ばなくなると最後の
     フレームが残り続けるので、フレームの通し番号が 0.15 秒進まなければ、
     Shear は無音の列を流し、Afterimage は塗りと線を消す

## 出荷後に変えられないもの

| | Invert | Shear | Afterimage |
|---|---|---|---|
| 名前（`truce.toml`） | `Invert` | `Shear` | `Afterimage` |
| `bundle_id` | `invert` | `shear` | `afterimage` |
| AU `fourcc` | `Invt` | `Shea` | `Aftr` |
| パラメーターのフィールド | `low`、`high`、`mix` | `shear`、`length`、`mix` | `persistence`、`damping`、`mix` |
| 版 | 1.0.0 | 1.0.0 | 1.0.0 |

リリースは Nonlinear Kit と同じ `series.toml` の `[kit]`。

## 単位

| ID | 単位 | 依存 | 状態 |
|---|---|---|---|
| KIT-5 | `nxe-spectral` の切り出し、`spectral-core`、Invert | KIT-1 | 🟡 コードは済み。Windows の基準画像と耳が残り |
| KIT-6 | Shear | KIT-5 | 🟡 同上 |
| KIT-7 | Afterimage | KIT-5 | 🟡 同上 |

付属プリセットの中身と既定値は、耳で決める仕事として残る。
