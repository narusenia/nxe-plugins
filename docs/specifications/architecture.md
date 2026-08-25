# アーキテクチャ

モノレポ全体の構造。プラグイン固有の設計は `plugins/<name>/docs/` にある。

## クレート構成

```
Cargo.toml                        ワークスペース
crates/
  nxe-ui/                         共通 Vizia ウィジェット・テーマ・Lucide アイコン
    examples/gallery.rs           ウィジェット一覧を単体アプリとして起動
plugins/
  doubler/
    doubler-core/                 DSP のみ（ホスト非依存）
    doubler/                      nih-plug ラッパ（パラメータ + UI）
    bundler.toml                  バンドルの表示名
    docs/                         このプラグインの要件・仕様・計画
xtask/                            nih_plug_xtask のバンドラ
```

`nxe-*` は共通クレートの接頭辞。プラグイン側のクレートは接頭辞を付けず
プラグイン名そのもの（`doubler`、`doubler-core`）とする。共通か固有かが
名前で判別できることを優先している。

## 依存の向き

```
doubler ──→ doubler-core      （DSP。nih-plug も Vizia も知らない）
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

## 共通 DSP クレートを今は作らない

遅延線・one-pole・スムージングノイズのような素材は `doubler-core` の中に置く。
`nxe-dsp` への抽出は**2 個目のプラグインが同じものを必要としたとき**に行う。
1 個目の時点では何が共通なのか分からないので、先に共通クレートを作ると
「1 つしか実装が無い抽象」が残る。

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

## 新しいプラグインを足す

1. `plugins/<name>/` に `<name>-core` と `<name>` の 2 クレートを作り、
   ワークスペースの `members` に追加する
2. `plugins/<name>/bundler.toml` に表示名を書く
3. `plugins/<name>/docs/requirements/REQ-<PREFIX>.md` を書く。ID は
   `REQ-<PREFIX>-<番号>`（Doubler は `DBL`）。要件が無い実装は始めない
4. DSP と UI の仕様を `plugins/<name>/docs/specifications/` に書く
5. 実装計画を `plugins/<name>/docs/implementation/<name>-plan.md` に書き、
   **実装単位を `docs/implementation/backlog.md` に行として追加する**。
   backlog に無い単位は着手対象として見つけられない
6. 順序に影響するなら `docs/implementation/roadmap.md` を直す
7. `docs/README.md` と `README.md` のプラグイン表に行を足す

共通ウィジェットが足りなければ `nxe-ui` に足す。そのとき
`examples/gallery.rs` にも同じ変更で追加する（`.agents/rules/vizia.md`）。
