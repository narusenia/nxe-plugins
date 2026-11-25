# アーキテクチャ

モノレポ全体の構造。プラグイン固有の設計は `plugins/<name>/docs/` にある。

## クレート構成

```
Cargo.toml                        ワークスペース（依存のピンはここに一元化）
truce.toml                        全プラグインの名前・ID・AU コード
series.toml                       リリースのシリーズと、それぞれに入るプラグイン
crates/
  nxe-audio/                      共通の音声処理（カーブ・オーバーサンプラ・
                                  biquad・エンベロープ・相対検出器）
  nxe-dsp/                        共通の解析（レベル・ステレオ像・スペクトラム）
  nxe-egui/                       共通 egui ウィジェット・テーマ・書体・NXE の印
                                  （truce を知らない）
    assets/                       書体（OFL の本文つき）と印の SVG
    examples/gallery.rs           ウィジェット一覧を eframe の単体アプリとして起動
patches/                          truce-clap / truce-egui の手直し（下）
plugins/
  doubler/
    doubler-core/                 DSP のみ（ホスト非依存）
    doubler/                      truce ラッパ（パラメータ + egui の窓）
    docs/                         このプラグインの要件・仕様・計画
```

**v2 で 6 本とも nih-plug + vizia から truce + egui に移った**
（`docs/implementation/v2-plan.md`）。v0 の共通ウィジェット `nxe-ui`、
nih-plug との結線 `nxe-plug-ui`、バンドラの xtask と `bundler.toml` は
その後に消した。v0 の vizia について分かったことは
`docs/archive/vizia-rules.md` と `docs/archive/vizia-pins.md` に残してある。

**truce-clap は patch を 2 つ当てて使っている**（`patches/truce-clap`、ルートの
`Cargo.toml` の `[patch.crates-io]`）。1 つ目: 6.3.0 は state / preset の読込で値を
変えたあとホストに `rescan(VALUES)` を送らず、clap-validator 0.4 の
`state-reproducibility-*` が落ちる。2 つ目: レイテンシが変わるとその場で
`latency.changed` を呼んでいた。CLAP はこれを `activate` の中でしか許さないので、
変化を覚えておき、動いていれば再起動を頼み、次の `activate` で知らせる
（レイテンシが動くのは Pumice だけ）。変更箇所は `nxe-plugins patch` で検索できる。
上流に入ったら patch ごと消す。

**truce-egui にも patch が 1 つある**（`patches/truce-egui`）。
`EguiEditor::with_font_definitions` を足し、複数の書体（Inter 2 ウェイトと
Geist Mono）を最初のパスの前に入れる。上流の `with_font` は 1 書体だけで、窓の
中から入れた書体は次のパスからしか効かないため、1 パスしか描かない
`screenshot!` の基準画像に自前の書体が写らなかった。もう 1 つ、描画の判定から
`has_requested_repaint()` を外した。遅延付きの再描画要求も「今すぐ」と扱われ、
窓が毎ティック描いていた（`docs/investigations/v2-frame-cost.md`）。

`truce.toml` も**ワークスペース直下に 1 つ**。`truce::plugin!` がコンパイル時に
クレート名で自分の `[[plugin]]` を探す。`au_manufacturer` と 5 本の `fourcc` は
`V2-1` でまとめて決めてある。

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

## 依存の向き

```
<plugin> ──→ <plugin>-core ──→ nxe-audio  （処理。truce も egui も知らない）
         ├─→ nxe-audio                    （ラッパが直接触る型だけ）
         ├─→ nxe-dsp                      （解析。同上）
         └─→ nxe-egui                     （ウィジェット。truce を知らない）
```

`nxe-audio` は `SPK-1` で `velour-core` から抜き出して作った。`velour-core` は
そこに依存し、`velour` も `Factor` のように**ラッパが直接名前を書く型**だけを
直接引く。互換のための再輸出は置いていない — どこから来た型かが `use` で
読めることを優先している（例外は測定ヘルパの `harmonics` 1 つで、テストが
書かれた場所に残すために `velour_core` が再輸出している）。

一方向で、戻る矢印は無い。この向きには 2 つの意味がある。

**`<plugin>-core` がホストを知らないこと。** v2 で nih-plug から truce に
移った時、DSP には触らずにラッパだけを取り替えられたのはこのため。同時に、DSP をホストなしでテストできるということでもある
（実際のテストはすべて core 側にある）。

**`nxe-egui` が truce を知らないこと。** ウィジェットは値を受け取り、
ポインタが何をしたかを返す。パラメータとの結線は各プラグインの `ui/bind.rs`
が持つ。この境界があると `examples/gallery` が truce を一切リンクせず単体で
起動できるので、DAW を立ち上げずに UI を反復できる。

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
doubler ──→ doubler-core      （DSP。truce も egui も知らない）
        ├─→ nxe-dsp           （解析。同上）
        └─→ nxe-egui          （ウィジェット。truce を知らない）
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

## 音声パスの FFT とレイテンシ申告 — Pumice の 2 つの例外

**2026-09-01。** どちらも Pumice（`REQ-PUM-007`）が初めてで、**規則を折って
いるのではなく、規則が想定していなかった場合**なので、理由をここに残す。

**2026-11-25 追記。** Spectral Kit（Invert / Shear / Afterimage）が 2 例目。
STFT は `crates/nxe-spectral` に移し、3 本とも `NORMAL` 固定で 1 ブロックを
申告する。乾いた音も変換を通るので、`MIX` = 0 はビット一致しない
（`spectral-kit-plan.md`）。Nonlinear Kit も乾いた音をオーバーサンプラに
通すので同じ（`nonlinear-kit-plan.md` の決定 5）。

### FFT は禁止されていない。「安いもので済むなら FFT を使わない」だった

過去に 2 回却下している。`nxe_dsp::spectrum` は「表示は対数軸で、FFT の
ビンは線形。20 Hz を分解するには 4096 点 = 85 ms 遅れる」、Sparkleur の
spectral flux は「FFT を音声パスに置くことになる（Velour で 48 バンドが
22 µs）」。**どちらも、より安い部品で同じ仕事ができたから却下している。**

Pumice は同じ仕事を安い部品でできない。共鳴 1 本を狙うには 20 Hz 級の
分解能が要り、それを対数バンクで作ると**フォロワがサンプルレートで回る**。
STFT は**ホップレートで回る**ので、分解能を 10 倍にしても安い。

**実測 30.6 µs / 予算 533**（`PUM-8`、`HIGH` / ステレオ / 512 サンプル /
48 kHz）。**ラインで 2 番目に安い** — Diorama 20.9、Pumice 30.6、Air 47.2、
Doubler 85、Velour 128、Sparkleur 129。**音声パスに FFT を置いた 1 本が
2 番目に安い**というのが、この例外を認める根拠そのもの。

**制約は変わらない。** `process()` で確保しない。バッファは最大 N = 16384 で
`initialize()` が確保し、`QUALITY` の変更は使う長さを変えるだけ。

### レイテンシ 0 は方針ではなく、5 本それぞれの判断だった

`REQ-DBL-006` / `REQ-SPK-012` / `REQ-AIR-012` / `REQ-DIO-012` /
`REQ-GLU-012` が**各自で**「報告 0」と決めている。**共通の制約として書かれた
ことは一度も無い。** Pumice は `N`（48 kHz `NORMAL` で 2048 サンプル =
42.7 ms）を申告する。

**未検証のホスト連携なので、ゲートとして扱う**（`PUM-1`）。4 ホストで
CLAP と VST3 の PDC が効くこと、および再生中の `QUALITY` 変更から復帰する
ことを、**DSP を 1 行も書く前に**確かめる。駄目なら方式ごと選び直す。

### 「`MIX` = 0 でビット一致」は Pumice では約束しない

5 製品が共通で持っていた約束（`REQ-GLU-001` が「5 製品で共通の透明性の
約束」と書いている）。**オーバーラップアドは全ビンのゲインが 1 でも浮動小数の
丸めが残る**ので、Pumice は成立させられない。

代わりに**すべて「入力との差が −120 dB 以下」で書く**（`REQ-PUM-012`）。
Pumice は dry を STFT に通さないので `MIX` = 0 は結果的にビット一致するが、
**受入条件はそれに依存しない** — dry ディレイラインを持つ理由は透明性では
なく、dry と wet が 32 ms ずれて `MIX` が櫛形になるのを防ぐことにある。

### `nih_plug::util::stft::StftHelper` を使わない

nih-plug は OLA のリングバッファを持っている。**使わない**（`REQ-PUM-015`）。
`nih_plug` は GPL 側なので、`pumice-core` が依存すると寛容側から移り、
**「将来 AU ラッパを別のフレームワークで書いたとき、同じコードを呼べる」**
という `<plugin>-core` の存在理由が消える。バッファリングがラッパ側にあると、
AU ラッパはエンジンではなく骨組みごと書き直すことになる。

**「自前のほうがレイテンシが 10 ms 短い」と書いていたが、それは誤りだった**
（2026-09-01 に訂正）。出力サンプルはそれを覆う最後のフレームが足されるまで
確定せず、そのフレームは `N − 1` サンプル先に終わる。**オーバーラップは
レイテンシを買い戻さない。** `StftHelper` がリング長 `N` を返しているのが
正しい。**理由はライセンスと AU の道だけで、それは単独で成立している。**

## ビルドと配布

truce に移ったプラグインは `cargo truce` でビルド・インストール・検証する。

```bash
cargo truce install --user --clap --vst3 --au2 -p doubler
cargo truce validate --clap --pluginval --auval -p doubler
```

CI（`check.yml` の `validate`）が macOS で同じ 2 行を、`series.toml` のシリーズごとのジョブに分けて並べて回す（clap-validator と auval が先、pluginval が後）。**リリースはシリーズ単位**
（`release.yml`）: タグ `<series>-<YYYY.MM>[.N][-beta.N]`（`basic-2026.12`）で、
`series.toml` にあるそのシリーズの全プラグインを、macOS は
`cargo truce package`（universal、CLAP / VST3 / AU v2。stage された bundle を
使い、installer は配らない）、Windows は `cargo truce build`（CLAP / VST3）で
作り、validate を通してからプラグインごとに zip にする。zip は全部 1 つの
draft release（`NXE <Series> <version>`）に付く。Linux は出さない。
プラグインのバージョンは各 `Cargo.toml` が持ち、リリースノートに並ぶ。

```bash
mise run install velour     # cargo truce install（CLAP / VST3 / AU）。先にホストを閉じる
mise run gallery            # 共通ウィジェットの gallery
mise run check              # fmt + clippy + test
```

**CI**: プルリクエストで `mise run check`、画面の基準画像との比較、validate。macOS は署名も公証もしないので、Gatekeeper の回避手順を README に
持つ（Apple Developer Program を契約したら署名を足す。それまでは手順が正）。

## 踏んだ罠

実装して分かったことは、それぞれ守るべき形として `.agents/rules/` に書いてある。
探す場所の地図だけここに置く。

| 罠 | 書いてある場所 |
|---|---|
| v0 の vizia の罠（相互排他の feature、CSS、レンズ） | `docs/archive/vizia-rules.md`、`docs/archive/vizia-pins.md` |
| CLAP の `latency.changed` は `activate` の中だけ | 上記「クレート構成」の truce-clap の patch |
| ブロック単位 API を使うと `REQ-DBL-012` が壊れる | `.agents/rules/rust.md` |
| `cargo clippy \| tail` で終了コードが潰れる | `.agents/rules/rust.md` |

## 新しいプラグインを足す

1. `plugins/<name>/` に `<name>-core` と `<name>` の 2 クレートを作り、
   ワークスペースの `members` に追加する
2. ワークスペース直下の `truce.toml` に `[[plugin]]` を足し（`name`、
   `bundle_id`、`fourcc`。表示名の規約は `AGENTS.md`）、出すシリーズの
   `series.toml` にクレート名を足す
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

共通ウィジェットが足りなければ `nxe-egui` に足す。そのとき
`examples/gallery.rs` にも同じ変更で追加する（`.agents/rules/ui.md`）。
