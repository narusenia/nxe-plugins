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

**規則は [`.agents/rules/ui.md`](../../.agents/rules/ui.md)**（何を守るか）と
[`.agents/rules/vizia.md`](../../.agents/rules/vizia.md)（この vizia が何をして
何をしないか）。ここは**どう使うか**。

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
| [`polar::PolarField`](src/polar.rs) | 半円上の点をドラッグする 2 軸フィールド。基準点（アンカー）も半径方向にドラッグできる。`PolarFieldModifiers` で `.highlight(lens)`（外から 1 点を指す）と `.density(lens)`（方向ごとの信号量を扇形で背後に敷く） | `impl Res<Vec<FieldPoint>>` ×2（点と基準点） | `Fn(&mut EventContext, FieldGesture)` |
| [`entry::ValueEntry`](src/entry.rs) | クリックで打ち込める数値 | `impl Lens<Target = String>`（表示文字列） | `Fn(&mut EventContext, &str)` |
| [`curve::CurveView`](src/curve.rs) | 曲線・帯・縦ドラッグのハンドル。`CurveViewModifiers` の `.analysis(lens)` で信号のカーブを背後に塗り、`.reference(lens)` で読み取りの基準線を差し替える（既定は窓の中央の水平線。入出力の伝達曲線は対角線に対して読む） | `impl Res<...>` ×3（曲線・帯・ハンドル） | `Fn(&mut EventContext, usize, Gesture)` |
| [`band::BandField`](src/band.rs) | 対数周波数のパネル。掴める帯域の区画と信号のカーブ 2 本。`BandFieldModifiers` で `.highlight(lens)`、`.focus(lens)`（下端のレールを横に引いて全区画をまとめて動かす）、`.unity(y)`（「変化なし」の線を引く。呼ばなければ区画は床から生える） | `impl Res<Vec<Band>>` + `impl Res<Curve>` ×2 | `Fn(&mut EventContext, BandGesture)` |
| [`meter::Meter`](src/meter.rs) | レベルバー 1 本とピークホールドの印。`new` が縦、`horizontal` が横。操作は無い | `impl Res<f32>` ×2（レベル・ホールド） | — |

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

`BandField` は [`band::BandGesture`](src/band.rs)。区画の分（`Begin` /
`Change { index, level }` / `End` / `Reset` / `Hover`）と、下端のレールの分
（`FocusBegin` / `FocusChange(f32)` / `FocusEnd` / `FocusReset`）。**レールは
`.focus(lens)` を繋いだときだけ生きる** — 書くものが無い呼び出し側は繋がなければ
よく、そのときレールは掴めるように見えない。

**縦ドラッグと横ドラッグを位置で見分けさせない。** 区画が全高になった瞬間に
曖昧になり、最初の数ピクセルの向きから推測するのはもっと悪い。だから横方向は
**常にそこにある下端のレール**が受ける。

`PolarField` だけは [`polar::FieldGesture`](src/polar.rs) を使う。2 つの値が
同時に動くため。点の分（`Begin` / `Change` / `End` / `Reset` / `Hover`）に加えて
アンカーの分（`AnchorBegin` / `AnchorChange(f32)` / `AnchorEnd` /
`AnchorReset`）がある。**アンカーは半径を共有し、角度では動かない** — 何本
立っていても表しているのは 1 つの値。書くものが無い呼び出し側は無視すればよく、
そのときアンカーは動かないだけ。

## アクセントの塗り

**塗りは 1 色ではなくグラデーション**（`ACCENT` → `ACCENT_WASH`）。同じ色相の
まま明度だけ動くので「アクセントは 1 色」の規則はそのままで、**淡いほうが
「遠い」**を意味する。

自前描画のウィジェットは `theme::accent_paint(x0, y0, x1, y1)` を使う。
`(x0, y0)` が**静止端**、`(x1, y1)` が**振り切った端**で、渡すのは
**トラック全体の範囲**（塗った部分ではない）。そうすると 1/4 まで塗ったバーは
ランプの 1/4 を見せるので、**値の違うバー同士が重なる範囲で同じ色**になる。

CSS 側は `.accent`（左→右）と `.accent-up`（下→上）。

**勾配は「量」にだけ使う。** バーがどこまで行ったか、メーターがどれだけ大きいか、
ノブがどこまで回ったか。**単に on / off の状態はフラットな `ACCENT`** —
淡い側に意味を持たせようがないし、**文字が乗ると自分の幅の中で
コントラストが変わって片端が読みにくくなる**（選択中のセグメントで実際にそう
なったので、この規則はそこから書いた）。同じ理由で曲線の描線もフラット:
軌跡であって量ではないし、暗い地の上では淡い側が単に明るいだけで
「右のほうが大事」に見える。

## ヘッダ

`nxe_ui::header::header(cx, "NXE SPARKLEUR", "five-band dynamics + sparkle")`。
ワードマーク・役割の一行・その下の `rule-accent` の 3 点セット。

**3 つのプラグインが同じものを 3 回書いていた**ので上げた。偶然同じなのと
意図して同じなのは別で、片方が罫線を欲しがった瞬間にずれる。

右の一行は**その窓が何のためのものか**。ホストのプラグイン一覧は名前しか
くれないが、窓が開いた時点で名前は既に知っている唯一のことなので、名前だけの
ヘッダは何も足していない。

## スイスの層

グリッドを見せるための小さな部品。

| クラス | 何 |
|---|---|
| `.eyebrow` | 区画の名前。9 px の `SUBTLE`。**コントロール名ではない**ので下のラベル列に混ざらない |
| `.heading` | `.eyebrow` を載せる器。下に 1 px の罫線が付く |
| `.readout` | その区画が見せるための 1 個の数字。**1 区画に 1 つまで** |
| `.rule` | 1 px の罫線。列の幅いっぱい |
| `.rule-accent` | 2 px のアクセント。**フラット** — 罫線は量ではないので勾配を持たせない（それに、消えていく端は「描き終わっていない」に見える） |

**角丸は 0 のまま。** グリッドは直線で描く。
`letter-spacing` と `line-height` はこの vizia に無いので、階層は
**サイズ・ウェイト・色・罫線**で作る（`.agents/rules/vizia.md`）。

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
- **設定はアクセント、信号はニュートラル。** 解析の重ね描き（`.density` /
  `.analysis`）は **`FOREGROUND` を薄く**（0.14〜0.16）敷く。何を設定したかと
  何が鳴っているかは、一目で区別できないと重ねる意味が無い
- **信号は「明るくする」層。** 中間色のグレーで塗ると、下に色が付いている
  ところで濁る。低い不透明度の光なら下を持ち上げるだけで済む
- 同種のものの組を見分けたいときは**色相を増やさず** `ACCENT_DEEP` と
  `ACCENT_BRIGHT` の間を `Token::mix` で刻む（`PolarField` の `FieldPoint::tint`）
- **角丸は無し**（`RADIUS_CONTROL` / `RADIUS_CARD` とも 0）。丸めようとすると
  コンパイル時アサーションで止まる。定数は残してあるので、気が変われば 1 行
- 間隔は 4px グリッドの 5 段（`SPACE_1`..`SPACE_5`）。この 5 つ以外を使わない
- 文字は 2 段（`FONT_LABEL` 12 / `FONT_VALUE` 10）と、ワードマーク用の `FONT_TITLE` 17。**CSS には単位を書かない** — この vizia は `font-size` に `px` を受け付けず、黙って既定の 16 に落ちる
- **ブラー・グロー・ガラス質感は使わない。** femtovg にブラーが無く、偽装すると
  素でやるより悪くなる。深さは値のコントラストと 1px の線だけで作る

### CSS のクラス

`.root` / `.panel` / `.section` / `.row` / `.divider` /
`.label` / `.value` / `.subtle` / `.disabled` / `.track` / `.accent` /
`.hoverable` / `.segmented` / `.segment` / `.icon`

Panel / Section / Row / Label / Divider をウィジェットにしていないのは、CSS の
クラスと vizia 組み込みの `Button` で足りるから。widget にすると数だけ増える。

**トグルボタンも新しいウィジェットは要らない。** `.segment` のスタイル
（`:checked` で accent）を単体の `Label` に当て、`checked` と `on_press` を
付ければトグルになる。

**押せる箱の中身には `.decoration` を付ける。** vizia は「マウスダウン時の
ホバー対象とマウスアップ時のホバー対象が同一のとき」だけ Press を発行するので、
中の Label がヒットテスト対象だと、ポインタが子をまたいだときに押せない
（「何回かクリックしないと反応しない」に見える）。`.decoration` は
`pointer-events: none` だけを持つクラス。

## フォント

[Geist](https://vercel.com/font)（SIL OFL 1.1）を埋め込んでいる。Sans の
Regular と Bold、Mono の Regular。この設計は階層を**サイズと色**で作り、
**太字はワードマーク 1 箇所だけの例外**。

- 既定は Geist Sans。`theme::install` が `set_default_font` で入れるので、
  普通の `Label` はそのまま Geist になる
- **数値は Geist Mono。** `font::value(cx, text)` を使う
- **プラグイン名は `font::title(cx, "NXE …")`。** ここだけ Bold。
  17 px の 1 ウェイトだとただのラベルに見えたので足した。**他のものに使わない** —
  2 つ目が要るなら「サイズと色で作る」という原則が間違っていたということなので、
  そのときは原則ごと書き換える

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
4. 描画は `View::draw` を書く前に CSS で足りないか考える。箱と文字で
   表現できるものは CSS の仕事
5. **同じ変更で [`examples/gallery.rs`](examples/gallery.rs) に並べる。**
   gallery に無いウィジェットは DAW を開かないとレビューできないので、
   レビューされない
6. 算術は純粋関数に切り出してテストする。描画と操作は目で見るしかないが、
   その下の計算はそうではない。**対や鏡のあるところに符号の間違いが住む**

## まだ無いもの

- `ToggleSwitch` — **2 個のプラグインがどちらも要らなかった。** `.segment` を
  当てた `Label` に `checked` と `on_press` で足りる（上記）。3 個目でも要らな
  ければ計画から落とす
