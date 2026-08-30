# 窓を開くと DAW がもたつく — 原因と対策

**2026-08-30。** 「全部のプラグインで、UI を開くと DAW 自体の操作がもっさり
する」という報告から始めた調査と、その対策の記録。

**結論を先に。** 窓 1 枚が **ホストの UI スレッドの 62 %** を食っていた。
Studio One で実測した数字で、推測ではない。原因は 1 つではなく 4 つあり、
4 つとも直した。

---

## 1. 何が起きていたか

### 実測（Fender Studio Pro 8.0.3 / NXE Velour VST3 / Apple Silicon / Retina）

`sample <pid> 5` をホストのプロセスに対して撮り、メインスレッドだけを見た。

| 状態 | main thread busy | うち `on_frame` |
|---|---|---|
| UI なし・停止 | 3 % | — |
| UI なし・再生（無音） | 8 % | — |
| UI なし・**実音声を再生** | 8 % | — |
| UI あり・停止 | 5 % | 1.2 % |
| UI あり・再生（無音） | 10 % | 1.1 % |
| **UI あり・実音声を再生** | **70 %** | **62.2 %** |

DAW 自身の仕事は一貫して 8 % 前後。**上乗せの 62 pt は丸ごとプラグイン**で、
66.7 Hz × 5 秒 = 333 フレームで割ると **1 フレームあたり 9.3 ms**。フレーム
予算 15 ms のうち 9.3 ms をホストの UI スレッドから奪っていた。

**無音だと 1.1 %。** 窓は「何かが変わったときだけ」描いている。つまり重いのは
「窓が開いていること」ではなく「**値が変わり続けること**」で、音が通っている
間だけ 62 % になる。

### なぜホストが遅くなるのか（構造）

`baseview` の macOS バックエンドは、窓を開いたスレッドのラン ループに
`CFRunLoopTimer` を刺す:

```rust
// baseview/src/macos/window.rs
let timer = CFRunLoopTimer::new(0.0, 0.015, 0, 0, timer_callback, &mut timer_context);
CFRunLoop::get_current().add_timer(&timer, kCFRunLoopDefaultMode);
```

プラグインにとってそのラン ループは **ホストのメインスレッド**。`on_frame` の
中身 — イベント処理・バインディング・スタイル・レイアウト・描画・GL swap —
は全部そこで同期実行される。プラグインが 62 % 使うのではなく、**DAW の UI
スレッドが 62 % 奪われる**。窓を増やせば足し算で効く。

### 62 % の内訳（メインスレッド比）

| | |
|---|---|
| `femtovg::Canvas::flush` → `OpenGl::render` | **45.4 %** |
| `flushBuffer`（GL swap） | 9.0 % |
| `on_frame_update` | 7.4 % |
| ├ `layout_system` / morphorm | 5.0 % / 3.8 % |
| └ cosmic-text 再シェイプ | 4.4 % |
| `binding_system` | 1.5 % |
| パス構築（`fill_path` + `stroke_path` + `draw_entity`） | **9.8 %** |

**幾何の計算ではなく GL への送信が全部だった。** 最も熱い葉:

```
AGX::RenderContext::encodeAndEmitRenderState     57
GLDContextRec::buildPipelineStateDescriptor      56
GLDContextRec::setRenderUniformBuffers           35
GLDContextRec::loadCurrentPipelinePrograms       21
```

Apple Silicon の OpenGL は Metal エミュレーション（`AppleMetalOpenGLRenderer`）
で、**draw call ごとにパイプライン状態を組み直す**。だから効くのは
「ピクセルを減らす」ではなく「**draw call を減らす**」。

---

## 2. 原因と対策

### 原因 1 — 読み値のレンズが毎フレーム走っていた（リポジトリ側）

5 本とも、読み値の帯をこう書いていた:

```rust
Ui::params.map(move |_| decibels(analysis.buses.read()[0]))
```

コメントには「ハートビートが 30 Hz でモデルを触るから再評価される」とあった。
**その理解が間違っていた。** `binding_system` はハートビートと無関係に
**毎フレーム全ストアを評価する**。だからこのレンズは 66.7 Hz で `String` を
作り直し、値が動くたびに窓全体を描き直していた。

**対策。** ハートビートでモデルにコピーし、レンズはそのフィールドを見る。
`BasicStore::update` が前回の `String` と比較するので、**印字された数字が
変わったときだけ**再描画になる — 30 Hz が上限で、読み値が落ち着いていれば
描き直さない。

同じ形が読み値以外にもあった。Velour と Sparkleur の帯域図（`bands_of` が
handoff を読んでいた）、Sparkleur の Advanced 表と伝達曲線の点。全部モデル
経由に変えた。

副作用として、フェーダーをドラッグしたときの図の追従が最大 33 ms 遅れる。
メーターが元からそうで、1 ハートビート分。

### 原因 2 — 見えないものを描いていた（vizia フォーク）

`View::draw` の既定実装は、**全ての view について**背景を塗り、枠を描き、
アウトラインを描いていた — それが透明でも、幅ゼロでも。木の大半は背景も枠も
持たないコンテナで、femtovg は `fill_path` / `stroke_path` のたびに
**必ず draw call を 1 つ出す**（`append_cmd` は何もマージしない）。

つまり view 1 つにつき、何も映さない draw call が最大 3 つ。おまけにその
ための箱パスを毎フレーム `build_path` で組んでいた。

**対策**（`narusenia/vizia` `nxe-2026-08-30`）:

- 背景が完全透明なら塗らない
- 枠の幅が 0 か色が透明なら描かない
- アウトラインの幅が 0 か色が透明なら描かない
- 上のどれも無い view は箱パスを**組まない**（`DrawContext::has_box_decoration`）

**実測: 1 フレームあたりの draw command が 456 → 219（−52 %）。**

### 原因 3 — フレーム タイマーが 66.7 Hz だった（baseview フォーク）

`CFRunLoopTimer` の間隔 0.015 s は 66.7 Hz。どの一般的なディスプレイより
速く、メーターを読むのに必要な速さより速い。しかもそれがホストのラン ループ
で回る。

**対策**（`narusenia/baseview` `nxe-2026-08-30`）: 30 Hz にした。プラグインの
解析ハートビートと同じレートで、**何が再描画を要求しても上限がここで効く**。

### 原因 4 — 誰も見ていない帯域バンクが回り続けていた（リポジトリ側）

解析の publish は `editor_state.is_open()` で切られていなかった。オーディオ
スレッドの話なので UI のもたつきとは無関係だが、Velour は 48 バンド × 2、
Sparkleur は 32 バンドの帯域バンクを、窓が閉じていても 1 サンプルごとに
回していた（Sparkleur で 15.2 µs / ブロック、Velour はその倍以上）。

**対策。** **スペクトラムだけ**を `is_open()` で切った。メーターは切らない —
フォロワ 1 つ分で安く、窓が開いた瞬間に正しくないと困る。閉じている間バンクは
止まるので、開いた直後はフォロワのリリース（250 ms）だけ図が落ち着くのを待つ。

---

## 3. 効果

### gallery（`NXE_GALLERY_HZ=30`、同一機・交互に 3 回ずつ）

| | CPU |
|---|---|
| 対策前（vizia `nxe-2026-08-27` / baseview upstream） | 22.6 / 22.4 / 22.9 % |
| **対策後**（両フォーク `nxe-2026-08-30`） | **10.2 / 10.5 / 10.0 %** |

**フォークの 2 つだけで −55 %。** アイドルの窓は 0.5 % のまま。

内訳としては、vizia の描画パッチが 22.6 → 16.2 %、baseview の 30 Hz が
16.2 → 10 % 前後。

### プラグイン側の見込み

gallery は原因 1 の形を持たないので測れない。ホストでの 62 % は
**66.7 Hz でフル再描画していた**数字なので:

- 原因 1（30 Hz 化）で redraw が半分 → 約 31 %
- 原因 2 + 3 で draw call が半分・タイマーが半分 → **10 % 台前半**

**これは見込みであって実測ではない。** 検証は下の手順で。

---

## 4. まだやっていないこと

### femtovg の draw call マージ（調査済み・未実装）

`femtovg` 0.7.1 の `Canvas::append_cmd` は **コマンドを一切マージしない**:

```rust
fn append_cmd(&mut self, cmd: Command) {
    self.commands.push(cmd);
}
```

`Command` は `drawables: Vec<Drawable>` を持っているので、**構造としては
マージできる**。直前のコマンドと種別・paint・image・glyph texture・合成
モードが一致するなら、新しいコマンドを積む代わりに `drawables` を足せばよい。

**実測: 対策後の gallery で 219 コマンド / フレーム、うち直前とマージ可能な
ものが 56（26 %）。** つまり保守的な隣接マージで draw call が 4 分の 1 減る。
それ以上は描画順の並べ替えが要り、重なる要素があると絵が変わるので安全では
ない。

femtovg は crates.io 版なので、やるならフォークが要る。**根治**（OpenGL を
やめて Metal / wgpu にする）は femtovg のバージョンを上げるところからで、
vizia が 0.7.1 に固定されている以上そちらの改修とセットになる。別件。

### 検証していない候補 — `setNeedsDisplay: YES`

`baseview` の `swap_buffers` は `flushBuffer` の直後に
`msg_send![self.view, setNeedsDisplay: YES]` を打つ。フレームを画面に出すのは
`flushBuffer` の仕事なので、これは毎フレーム AppKit に不要な display cycle を
頼んでいる可能性がある — しかもプラグインでは**ホストのウィンドウ**に対して。

外して測ったが、**単独ウィンドウの gallery では差が出なかった**（効くとしたら
ホストのウィンドウ階層の中でだけなので、gallery では原理的に見えない）。
描画が壊れないことを目視で確認できなかったので**戻した**。DAW で試すなら
`narusenia/baseview` でその 1 行を消して測る。

### 直さなかったもの — ウィジェットの `cx.needs_redraw()`

`meter.rs` などが値を受け取るたび無条件に `needs_redraw()` を呼んでいる、と
最初は読んだ。**間違い**で、これらのイベントは `Handle::bind` から来るので
`BasicStore` が既に値を比較しており、変わったときしか届かない。値ガードを
足しても効かない。

### 直さなかったもの — Doubler の Detail 表

`detail.rs` はパラメータから `String` を毎フレーム組んでいる（8 ボイス ×
4 列）。値が動くのはユーザーが触ったときだけなので再描画は誘発しないが、
毎フレーム 32 個のアロケーションではある。読み値と同じ形なので、気になったら
同じ直し方で。

---

## 5. 測り方

**アイドルの窓はコストゼロのはず**、が元からの規則
（`.agents/rules/vizia.md`）。**動いている窓**は測られていなかった。

```bash
# アイドル（従来のテスト）
cargo run --release -p nxe-ui --example gallery

# 動いている窓 — プラグインが音を受けている状態に相当
NXE_GALLERY_HZ=30 cargo run --release -p nxe-ui --example gallery
```

CPU は起動分が混ざらないよう、開始後しばらくしてから差分で取る:

```bash
ps -o cputime= -p <pid>   # 2 回取って割る
```

ホストで測るときは、**プラグインの窓を前面に開き、実際に音を通した状態で**
撮る。無音だと 1 % しか出ないので、何も分からない:

```bash
sample <daw-pid> 5 -f ~/Desktop/open.txt
grep -c "vizia\|baseview\|femtovg" ~/Desktop/open.txt   # 0 なら撮れていない
```

---

## 6. 検証してほしいこと

1. `mise run gallery` を開いて、**絵が今までどおりか目で見る**。原因 2 の
   パッチは「透明なら描かない」なので、透明のつもりで見えていたものがあれば
   ここで消える。
2. `mise run install velour` を入れて Studio One で開き、再生しながら
   `sample` を撮る。**62 % がどこまで下がったか**が答え。
3. フェーダーのドラッグで図の追従が気にならないか（原因 1 の 33 ms）。

---

## 7. 参照

- 対策前の生サンプル: Studio One（`daw-ui-open-acutually-playing-vocal-sample`
  ほか 6 枚）と Ableton Live 12.4.5b11。どちらも手元の `~/Desktop`
- フォーク: [`narusenia/vizia`](https://github.com/narusenia/vizia)
  `nxe-2026-08-30` / [`narusenia/baseview`](https://github.com/narusenia/baseview)
  `nxe-2026-08-30`。どちらも上流に投げる価値がある。まだ投げていない
- 経緯と踏んだ罠: [`../../.agents/rules/vizia.md`](../../.agents/rules/vizia.md)
