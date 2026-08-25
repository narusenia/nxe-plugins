# nxe-ui

`nxe-plugins` の全プラグインが共有する Vizia のウィジェットとテーマ。

**このクレートは nih-plug を知らない。** ウィジェットは値とコールバックしか
受け取らず、パラメータとの結線は各プラグインが持つアダプタの仕事。この境界が
あるおかげで、`examples/gallery` が単体のデスクトップアプリとして起動でき、
DAW を開かずに UI を反復できる。

```bash
mise run gallery
```

守るべき規約は [`.agents/rules/vizia.md`](../../.agents/rules/vizia.md)。
**このリビジョンの vizia で何ができて何ができないか**もそこに書いてある（先に
読むと時間が節約できる）。

## 使いはじめ

ウィンドウを組むとき最初に 1 回、テーマとアイコンフォントを入れる。

```rust
use nxe_ui::theme;

Application::new(|cx| {
    theme::install(cx);   // スタイルシート + Lucide フォント
    // ...
})
```

## ウィジェット

| ウィジェット | 用途 | 値の受け取り方 | コールバック |
|---|---|---|---|
| [`knob::Knob`](src/knob.rs) | 回転コントロール | `impl Res<f32>` | `Fn(&mut EventContext, Gesture)` |
| [`bar::Bar`](src/bar.rs) | 横方向の細いスライダ。`new` は左端から、`bipolar` は中央から伸びる | `impl Res<f32>` | 同上 |
| [`segmented::SegmentedControl`](src/segmented.rs) | 排他選択のボタン列 | `Lens<Target = usize>` | `Fn(&mut EventContext, usize)` |
| [`polar::PolarField`](src/polar.rs) | 半円上の点をドラッグする 2 軸フィールド | `impl Res<Vec<FieldPoint>>` ×2（点と基準点） | `Fn(&mut EventContext, FieldGesture)` |
| [`curve::CurveView`](src/curve.rs) | 曲線・帯・縦ドラッグのハンドル | `impl Res<...>` ×3（曲線・帯・ハンドル） | `Fn(&mut EventContext, usize, Gesture)` |

値はすべて**正規化**（`0..=1`。双極のものは `0.5` が中央）。単位の写像は
呼び出し側が持つ。

### `Res` を取るものと `Lens` を要求するものがある

ほとんどは `impl Res<T>` — 静的な値でもレンズでも渡せる vizia の仕組みで、
プラグインはレンズを渡してオートメーション由来の更新を受け取れる。

`SegmentedControl` だけ `Lens` を要求する。各セグメントが「自分が選択中か」と
いう反応状態を個別に持つ必要があり、それには選択値をセグメントごとに map しな
いといけない。`Res` にその能力は無い。

### ジェスチャー

[`input::Gesture`](src/input.rs) は `Begin` / `Change(f32)` / `End` / `Reset` /
`Edit` の 5 つ。**`Begin` と `End` はプラグインがホストに伝えるためにある** —
これが無いとホストがオートメーションの操作を「1 回の編集」ではなく無関係な点の
散らばりとして記録する。`Reset` と `Edit` はウィジェットが「何が既定値か」「どう
入力させるか」を知らないので、呼び出し側が処理する。

操作はどのウィジェットでも同じ。

| 操作 | 効果 |
|---|---|
| 縦ドラッグ | 変更。全域で 200 px |
| `Shift` + ドラッグ | 5 分の 1 の速さ |
| ダブルクリック | `Reset` を通知 |
| `Cmd`（または `Ctrl`）+ クリック | `Edit` を通知 |

`Bar` も**縦**ドラッグ。横バーを横に引くのは一見自然だが、行を積んだときに端を
越えて隣に入る誤操作が起きる。

## テーマ

**[`theme`](src/theme.rs) の Rust 定数が正で、CSS はそこから生成する。**
カスタム描画のウィジェットは色を値として必要とするので、CSS を正にすると必ず
二重管理になる。

```rust
theme::ACCENT.vg()      // View::draw 用（femtovg）
theme::ACCENT.vizia()   // vizia の Color が要るところ
theme::ACCENT.css()     // 生成される CSS 用
```

- **色のリテラルを書かない。** CSS にも `View::draw` にも。生成された CSS に
  16 進の色が含まれないことをテストで固定してある
- **面と文字はニュートラル**（RGB の 3 チャネルが等しい）。アクセントだけが色を
  持つ。これもテストで固定
- 角丸は `RADIUS_CONTROL` = 2px / `RADIUS_CARD` = 3px。**角張ったデザイン。**
  丸めようとするとコンパイル時アサーションで止まる
- 間隔は 4px グリッドの 5 段（`SPACE_1`..`SPACE_5`）。この 5 つ以外を使わない
- 文字は 2 段（`FONT_LABEL` 12px / `FONT_VALUE` 13px）
- **ブラー・グロー・ガラス質感は使わない。** femtovg にブラーが無く、偽装すると
  素でやるより悪くなる。深さは値のコントラストと 1px の線だけで作る

### CSS のクラス

`.root` / `.panel` / `.panel-highlight` / `.section` / `.row` / `.divider` /
`.label` / `.value` / `.subtle` / `.disabled` / `.track` / `.accent` /
`.hoverable` / `.segmented` / `.segment` / `.icon`

Panel / Section / Row / Label / Divider をウィジェットにしていないのは、CSS の
クラスと vizia 組み込みの `Button` で足りるから。widget にすると数だけ増える。

**トグルボタンも新しいウィジェットは要らない。** `.segment` のスタイル
（`:checked` で accent）を単体の `Label` に当て、`checked` と `on_press` を
付ければトグルになる。

## フォント

[Geist](https://vercel.com/font)（SIL OFL 1.1）を埋め込んでいる。Sans と Mono の
Regular 1 ウェイトずつ。この設計は階層を**サイズと色**で作るので、他のウェイトは
使わない。

- 既定は Geist Sans。`theme::install` が `set_default_font` で入れるので、
  普通の `Label` はそのまま Geist になる
- **数値は Geist Mono。** `font::value(cx, text)` を使う

```rust
font::value(cx, lens.map(|v| format!("{v:.1}")));
```

小数桁を固定しても、プロポーショナルな字形では `1` と `8` で幅が違うので、
ノブをドラッグしている間に数字が横に揺れる。等幅にすればそれが根本的に消える。

ライセンス文はフォントの隣（`assets/geist/`）。**バイナリに焼き込まれるので
リリースのバンドルにも同梱が必要。**

## アイコン

Lucide の埋め込みフォント。2035 個すべてが定数になっている。

```rust
use nxe_ui::icon;

icon::label(cx, icon::CHEVRON_DOWN).font_size(20.0);
```

- **`icon::label` で作る。CSS で `font-family` を書かない** — このリビジョンの
  vizia ではスタイルシートの `font-family` が埋め込みフォントを選ばず、
  私用領域のコードポイントが無関係な CJK グリフとして描かれる
- 生のエスケープをビューに書かない。定数を使う
- **線幅は変えられない**（ストロークをグリフ化したもの）。太さが要るアイコンは
  `usvg` でパス化して `View::draw` で描く — 記録された例外であって既定ではない
- Lucide の更新は `mise run icons:generate`。手順は
  [`scripts/generate-icons.py`](../../scripts/generate-icons.py) の docstring

## ウィジェットを足す

1. `src/<name>.rs` に置き、`src/lib.rs` で公開する
2. 値は `impl Res<T>`、通知はコールバック。**パラメータを知ってはいけない**
3. 入力のふるまいは [`input`](src/input.rs) を使う。ドラッグの算術を再実装しない
4. 描画は `View::draw` を書く前に CSS で足りないか考える。角丸の箱と文字で
   表現できるものは CSS の仕事
5. **同じ変更で [`examples/gallery.rs`](examples/gallery.rs) に並べる。**
   gallery に無いウィジェットは DAW を開かないとレビューできないので、
   レビューされない
6. 算術は純粋関数に切り出してテストする。描画と操作は目で見るしかないが、
   その下の計算はそうではない。**対や鏡のあるところに符号の間違いが住む**

## まだ無いもの

- ツールチップ（vizia に `Tooltip` view はある）
- `SegmentedControl` のキーボード左右移動
- `Meter` / `ToggleSwitch` — 使うプラグインが出てから
