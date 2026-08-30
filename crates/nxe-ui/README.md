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
use nxe_ui::theme::{self, Palette};

Application::new(|cx| {
    theme::install(cx, Palette::SPARKLEUR);   // 書体 + アイコン + パレット + CSS
    // ...
})
```

**パレットは本ごとに 1 つ。** `Palette::DOUBLER` / `VELOUR` / `SPARKLEUR` /
`AIR` / `PARALLAX` の 5 つがあり、**色相だけが違う** — OKLCH の明度と彩度は
stop ごとに揃えてあるので、半分まで塗ったバーはどの窓でも同じ重さに見える。
テストが固定している（`theme::tests::the_palettes_are_one_family`）。

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

**塗りはフラット**（`UI-21`）。**長さが量**で、その上を走る階調は同じことを
2 回言っている。値の違うバーが 4 本並ぶと、階調版はどれも右端が同じ淡い色で
終わるので、**終端の見た目からは値が分からなかった**。

**自前描画のウィジェットは描画時にパレットを引く。**

```rust
fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
    let palette = theme::palette(cx);
    // ... palette.accent / palette.bright / palette.deep / palette.dim
    canvas.fill_path(&fill, &vg::Paint::color(palette.accent.vg()));
}
```

`theme::palette` は木を遡って**一番近い `Palette` モデル**を返す。`install` が
窓の根に 1 つ建てるので、通常はそれが見つかる。**`DataContext` を取るので、
組み立て中（`Context`）・イベント中（`EventContext`）・描画中（`DrawContext`）の
どれでも同じ呼び方**でよい。

**部分木に別のパレットを建てると、その下の自前描画だけ色が変わる。**
gallery の PALETTES パネルが 5 色を同時に見せているのはこれで、プラグインは
使わない道。**CSS 側は切り替わらない** — vizia はスタイルシートを差し替える
手段を持たないので、生成する CSS は `install` に渡した 1 つぶんだけ
（gallery は `NXE_GALLERY_PALETTE` で選ぶ）。

**`bright` / `deep` は塗りではなく「種類の見分け」に使う。** 同種のものが
何本か並ぶとき（ボイスの組、帯域）だけ `Token::mix` で刻む。

CSS 側は `.accent` 1 つ。**フラットなので向きが無く、横にも縦にも同じものを
使う。** 色は `install` に渡したパレットで焼き込まれている。

## 反転面

**1 つの窓に 1 枚だけ**、地がアクセントの面を置ける（`.agents/rules/ui.md`）。
**実際にはステータスバー**（`nxe_ui::status`）がそれで、下記は下敷きの仕組み。

**図には使わない。** Sparkleur で 1 度やって、**隣の伝達曲線が真っ黒になった** —
図は自前描画と CSS の両方でできていて、**入れ子のパレットに従うのは描画だけ**。
描線は反転して黒になり、地は `.panel` のまま黒だった。**地と語だけでできた面**
なら食い違う半分が無い。

```rust
use nxe_ui::surface;

surface::inverted(cx, |cx| {
    Label::new(cx, "FIELD").class("eyebrow").class("ink-muted");
    // 図
});
```

`surface::inverted` は `Palette::inverted()` を部分木のモデルとして建てるので、
**中のウィジェットは何も知らなくてよい** — いつもどおり `theme::palette(cx)` を
引けば、地とインクが入れ替わった色が返る。

**文字だけは例外で、自分で言う必要がある**（`.ink` / `.ink-muted`）。生成する
CSS は平坦で「この面の中のラベル」を書く手段が無いため、付け忘れるとアクセントの
上にほぼ白が乗る。**付け忘れが目に見える程度の語数**しか置かないこと。

**塗りも状態も曲線も罫線も、全部フラット。** 以前は「勾配は量、状態はフラット」
の 2 本立てで、その境界で 1 度失敗している（選択中のセグメントの文字が自分の幅の
中でコントラストを変えて読みにくくなった）。**境界を無くしたので規則が 1 本
減った。**

## ステータスバー

窓の**下端に敷く帯**。ホバー中のコントロールの 1 行がここに出る。

```rust
nxe_ui::status::bar(cx, "five-band dynamics + sparkle");
```

**窓幅いっぱいに、端まで**。手前で切れると「窓の床」ではなく「もう 1 枚の
パネル」に見える。高さは `status::HEIGHT`（窓の高さの足し算の一部）。

**最初はヘッダの右に置いていた。** eyebrow の大きさで黒地の上、しかもポインタが
居る場所から一番遠い端 — **読みにくかった**。自分の帯を持たせると、色地の上の
1 行になって窓の中で一番読みやすいものになる。

## ヘッダ

```rust
nxe_ui::header::header(cx, "Sparkleur", "five-band dynamics + sparkle", |cx| {
    // MODE を持つ本だけここに置く。持たない本は空のクロージャ
});
```

**ワードマークは製品名だけ。** `NXE` は左に小さく別で出る。ホストのリストは
窓を開く前に読み終わっているので、窓の中で製品名の前にベンダーを繰り返す意味が
無い。

**帯の右は窓が何のためのものか**（`role`）。ポインタの 1 行は下の
ステータスバーに出る。

```rust
use nxe_ui::hint::Describe;

Knob::new(cx, lens, gesture).describe("how hard the curve is driven");
```

説明は**呼び出し側が書く** — `Knob` は自分が `DRIVE` だと知らない。**短く保つ**:
切り詰めの手段が無いので、長すぎる文はワードマークを押しのける。

**3 つのプラグインが同じものを 3 回書いていた**ので上げた。偶然同じなのと
意図して同じなのは別で、片方が罫線を欲しがった瞬間にずれる。

## スイスの層

グリッドを見せるための小さな部品。

| クラス | 何 |
|---|---|
| `.eyebrow` | 区画の名前。9 px の `SUBTLE`。**コントロール名ではない**ので下のラベル列に混ざらない |
| `.heading` | `.eyebrow` を載せる器。下に 1 px の罫線が付く |
| `.readout` | その区画が見せるための 1 個の数字。**1 区画に 1 つまで** |
| `.rule` | 1 px の罫線。列の幅いっぱい |

**角丸は 0 のまま。** グリッドは直線で描く。
`letter-spacing` と `line-height` はこの vizia に無いので、階層は
**サイズ・ウェイト・色・罫線**で作る（`.agents/rules/vizia.md`）。

## テーマ

**[`theme`](src/theme.rs) の Rust 定数が正で、CSS はそこから生成する。**
カスタム描画のウィジェットは色を値として必要とするので、CSS を正にすると必ず
二重管理になる。

```rust
theme::palette(cx).accent.vg()      // View::draw 用（femtovg）
theme::palette(cx).accent.vizia()   // vizia の Color が要るところ
theme::Palette::AIR.accent.css()    // 生成される CSS 用
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
- 同種のものの組を見分けたいときは**色相を増やさず** `palette.deep` と
  `palette.bright` の間を `Token::mix` で刻む（`PolarField` の `FieldPoint::tint`）
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

語は [Inter](https://rsms.me/inter/)、数値は
[Geist Mono](https://vercel.com/font)（どちらも SIL OFL 1.1）。Inter は
Light と Regular の 2 面。この設計は階層を**サイズと色**で作り、
**ウェイトが意味を持つのはワードマーク 1 箇所だけ**。

- 既定は Inter Regular。`theme::install` が `set_default_font` で入れるので、
  普通の `Label` はそのまま Inter になる
- **数値は Geist Mono。** `font::value(cx, text)` を使う。Inter の tabular
  figures（`tnum`）は OpenType feature で、**この vizia には feature を
  立てる道が無い**
- **プラグイン名は `font::title(cx, "Sparkleur")`。** ここだけ Light、26 px。
  **`NXE` は付けない** — ベンダーは左に小さく別で置く（`nxe_ui::header`）
- **Light をラベルの大きさに使わない。** 小さい Light は静かではなく細いだけで、
  暗い地の上では上品になる前に脆くなる（17 px で実際にそう見えた）
- **2 つ目のウェイトの用途を作らない。** 要るなら「サイズと色で作る」という
  原則が間違っていたということなので、そのときは原則ごと書き換える

```rust
font::value(cx, lens.map(|v| format!("{v:.1}")));
```

**その `lens` はモデルのフィールドであること。** オーディオスレッドの
`Handoff` をレンズの中で読んではいけない — `binding_system` はハートビートと
無関係に**毎フレーム全ストアを評価する**ので、そういうレンズはフレームレート
で `String` を作り直し、値が動くたびに窓全体を描き直す。ハートビートで
モデルに書き、レンズはそれを見る（`nxe_ui::readout` の「Give it a value from
the model」、[調査](../../docs/investigations/ui-frame-cost.md)）。

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
