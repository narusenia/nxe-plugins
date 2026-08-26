# アーキテクチャ

モノレポ全体の構造。プラグイン固有の設計は `plugins/<name>/docs/` にある。

## クレート構成

```
Cargo.toml                        ワークスペース（依存のピンはここに一元化）
bundler.toml                      バンドルの表示名（パッケージ名でセクション分け）
.cargo/config.toml                `cargo xtask` のエイリアス
crates/
  nxe-audio/                      共通の音声処理（カーブ・オーバーサンプラ・
                                  biquad・エンベロープ・相対検出器）
  nxe-dsp/                        共通の解析（レベル・ステレオ像・スペクトラム）
  nxe-ui/                         共通 Vizia ウィジェット・テーマ・Lucide アイコン
    examples/gallery.rs           ウィジェット一覧を単体アプリとして起動
  nxe-plug-ui/                    nih-plug のパラメータと nxe-ui の結線
plugins/
  doubler/
    doubler-core/                 DSP のみ（ホスト非依存）
    doubler/                      nih-plug ラッパ（パラメータ + UI）
    docs/                         このプラグインの要件・仕様・計画
xtask/                            nih_plug_xtask のバンドラ
```

`bundler.toml` は**ワークスペースの直下に 1 つ**。バンドラがそこを読むので、
プラグインごとに置くことはできない。

`nxe-*` は共通クレートの接頭辞。プラグイン側のクレートは接頭辞を付けず
プラグイン名そのもの（`doubler`、`doubler-core`）とする。共通か固有かが
名前で判別できることを優先している。

### `nxe-audio` と `nxe-dsp` を分けている理由

どちらもホスト非依存・確保なし・音声スレッドで動く。分けているのは**リスクの
階級が違う**から。

- **`nxe-dsp` は音を変えない。** doc に「None of it changes the audio」と
  書いてあるのが契約。ここのバグは**絵が壊れるだけ**
- **`nxe-audio` は音そのもの。** ここのバグは**音が壊れる**

同じ箱に入れると、その差が消えて「どっちに入れるのか」を毎回考えることになる。
`Handoff` を挟んで解析が音声スレッドを止められない構造にしたのと同じ理由。

### `nxe-plug-ui` が別クレートである理由

`nxe-ui` は**nih-plug を知らない**（下の「依存の向き」）。パラメータとの結線は
nih-plug と vizia の両方を知る必要があるので、**3 つ目のクレート**になる。
`nxe-ui` に入れると `examples/gallery` が nih-plug をリンクして単体起動できなく
なり、**DAW を開かずに UI を反復する**という土台が消える。

Doubler と Velour は同内容の `param_bind.rs` をそれぞれ持っていた。
**3 個目（Sparkleur）が要求した時点で上げる**（下の「共通クレートに上げる
タイミング」）。

## 依存の向き

```
doubler ──→ doubler-core      （DSP。nih-plug も Vizia も知らない）
        ├─→ nxe-dsp           （解析。同上）
        └─→ nxe-ui            （ウィジェット。nih-plug を知らない）
```

一方向で、戻る矢印は無い。この向きには 2 つの意味がある。

**`<plugin>-core` がホストを知らないこと。** nih-plug には AU ラッパが無い。
将来 AU を出したくなったとき、別フレームワークのラッパから同じ DSP を呼べる
必要がある。同時に、DSP をホストなしでテストできるということでもある
（実際のテストはすべて core 側にある）。

**`nxe-ui` が nih-plug を知らないこと。** ウィジェットは値とコールバックだけを
受け取る。パラメータとの結線はプラグイン側の薄いアダプタが持つ。この境界が
あると `examples/gallery` が nih-plug を一切リンクせず単体で起動できるので、
DAW を立ち上げずに UI を反復できる。`nih_plug_vizia` のパラメータ binding は
便利だが、それを `nxe-ui` に入れると gallery が成立しなくなる。

## 依存のピン

nih-plug は crates.io に無く git 依存。`nih_plug` / `nih_plug_vizia` /
`nih_plug_xtask` は同一リポジトリなので、**3 つを同じ revision に揃える**。
後者 2 つは `nih_plug` を path で参照しているため、revision がずれると
`nih_plug` がツリーに 2 つ入る。

**vizia は `nih_plug_vizia` が使っているものと完全に同一でなければならない。**

```toml
vizia = { git = "https://github.com/robbert-vdh/vizia.git", tag = "patched-2024-05-06" }
```

Robbert のフォークの固定タグで、上流の vizia ではない。Cargo は git ソースや
revision が違えば**別のクレート**として扱うので、`nxe-ui` が別の vizia を
引くと、そこで作ったビューが `nih_plug_vizia` が受け取る型と一致せず何も
コンパイルできない。ワークスペースの `[workspace.dependencies]` に 1 箇所だけ
書き、各クレートは `workspace = true` で参照する。

この制約には副作用が 2 つある。

**上流 vizia の新機能は来ない。** ピンは 2024-05-06 のタグで、それ以降に
上流へ入ったものは使えない。SVG ビューのような後発の機能を前提に設計しない
（アイコンをフォントで解決しているのはこれが理由 —
`plugins/doubler/docs/specifications/ui.md`）。

**`winit` を有効にしてはいけない。** vizia の 2 つのバックエンドは
**相互排他**で、`Application` の re-export が
`cfg(all(not(feature = "winit"), feature = "baseview"))` とその鏡像で守られて
いる。両方を有効にすると**どちらも** re-export されず、`Application` を使う
コードが 1 つもコンパイルできない（`nih_plug_vizia` 自身も含む）。

Cargo の feature はグラフ全体で加算的で、クレート単位で無効化できない。
dev-dependency に隔離する手も効かない — resolver v2 は dev ターゲットを
ビルドする時点で dev-dependency の feature を通常グラフに統合するので、
`cargo check --workspace --all-targets` で衝突が復活する。

したがって答えは 1 つだけ。**ワークスペース全体が `baseview` を使い、`winit`
はどこでも有効にしない。** `nxe-ui` の gallery は baseview の単体ウィンドウを
開く（`Application::run`）。副作用として、gallery が**プラグインと同じ
バックエンド**で動くことになる — UI の挙動を DAW の外で見る意味が強くなる。

## 共通 DSP クレート `nxe-dsp`

**解析だけが入る。** 音そのものを作る部品（遅延線・one-pole・スムージング
ノイズ）は今も `doubler-core` の中にあり、抽出は 2 個目のプラグインが同じものを
必要としたときに行う — 1 個目の時点では何が共通なのか分からず、先に作ると
「1 つしか実装が無い抽象」が残るから。

**解析は例外**（2026-08-26）。レベル・ステレオ像・スペクトラムは
**どのプラグインでも同じもの**で、何が共通かを考える必要が無い。作る前から
再利用が決まっているものに YAGNI を当てても、置き場所が 1 個目のプラグインの
中になるだけで得は無い。

`nxe-dsp` の制約は `<plugin>-core` と同じ。**ホスト API も UI ツールキットも
リンクしない。** 音声スレッドで走り、素の数値を渡す。だからウィンドウ無しで
テストできる。

```
doubler ──→ doubler-core      （DSP。nih-plug も Vizia も知らない）
        ├─→ nxe-dsp           （解析。同上）
        └─→ nxe-ui            （ウィジェット。nih-plug を知らない）
```

**音声スレッドから UI への受け渡しは `Handoff`。** `AtomicU32` に `f32` の
ビットを置くだけの latest-wins で、ロックもバッファの入れ替えも無い。読み側は
60 Hz の再描画なので、2 回の書き込みが混ざったフレームを 1 枚見ることはあっても
誰にも見えない。整合性を買うにはロックか三重バッファが要り、音声スレッドは
どちらも待てない（`REQ-DBL-011`）。

## 音声スレッドの規則

`.agents/rules/rust.md` が正。要点だけ:

- `process()` で確保しない。バッファは `initialize()` で最大ブロックサイズと
  サンプルレートが分かった時点で確保する
- 音声スレッドでロック・ブロッキング・I/O をしない
- パラメータはホストからの入力なので clamp する。panic させない

## ビルドと配布

バンドルは nih-plug の xtask が行う。

```bash
mise run bundle doubler     # cargo xtask bundle doubler --release
mise run install doubler    # バンドルして ~/Library/Audio/Plug-Ins へコピー（macOS）
mise run gallery            # 共通ウィジェットの gallery
mise run check              # fmt + clippy + test
```

出力は `target/bundled/` に `<name>.clap` と `<name>.vst3`。表示名は
`plugins/<name>/bundler.toml` が決める。

**CI**: プルリクエストで `mise run check`。タグを打つと macOS（universal）・
Windows・Linux でバンドルし、プラットフォームごとの zip を GitHub Release に
添付する。macOS は署名も公証もしないので、Gatekeeper の回避手順を README に
持つ（Apple Developer Program を契約したら署名を足す。それまでは手順が正）。

## 踏んだ罠

実装して分かったことは、それぞれ守るべき形として `.agents/rules/` に書いてある。
探す場所の地図だけここに置く。

| 罠 | 書いてある場所 |
|---|---|
| vizia の `winit` / `baseview` が相互排他 | 上記「依存のピン」と `.agents/rules/vizia.md` |
| CSS の `font-family` が埋め込みフォントを選ばない | `.agents/rules/vizia.md` |
| `draw_text` が view 自身のテキストしか描けない | `.agents/rules/vizia.md` |
| vizia の既定の文字色が黒 | `.agents/rules/vizia.md` |
| CSS のプロパティ名が web と違う（`child-space` 等） | `.agents/rules/vizia.md` |
| レンズの `map` は 1 フィールドしか見られない | `.agents/rules/vizia.md` |
| `bundler.toml` はワークスペース直下に 1 つ | 上記「クレート構成」 |
| ブロック単位 API を使うと `REQ-DBL-012` が壊れる | `.agents/rules/rust.md` |
| `cargo clippy \| tail` で終了コードが潰れる | `.agents/rules/rust.md` |

## 新しいプラグインを足す

1. `plugins/<name>/` に `<name>-core` と `<name>` の 2 クレートを作り、
   ワークスペースの `members` に追加する
2. ワークスペース直下の `bundler.toml` に `[<パッケージ名>] name = "NXE <名前>"` を足す
   （表示名の規約は `AGENTS.md`。クレート名は接頭辞なしの素の名前）
3. `plugins/<name>/docs/requirements/REQ-<PREFIX>.md` を書く。ID は
   `REQ-<PREFIX>-<番号>`（Doubler は `DBL`）。要件が無い実装は始めない
4. DSP と UI の仕様を `plugins/<name>/docs/specifications/` に書く
5. 実装計画を `plugins/<name>/docs/implementation/<name>-plan.md` に書き、
   **実装単位を `docs/implementation/backlog.md` に行として追加する**。
   backlog に無い単位は着手対象として見つけられない
6. 順序に影響するなら `docs/implementation/roadmap.md` を直す
7. `docs/README.md` と `README.md` のプラグイン表に行を足す
8. **共通クレートに上げるものがあれば、その単位を計画の最初に置く。**
   `nxe-audio` は Sparkleur が要求して生まれた（`REQ-SPK-015`）

共通ウィジェットが足りなければ `nxe-ui` に足す。そのとき
`examples/gallery.rs` にも同じ変更で追加する（`.agents/rules/vizia.md`）。
