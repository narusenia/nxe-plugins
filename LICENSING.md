# ライセンス

**このリポジトリは 1 つのライセンスでは説明できない。** 再利用できる部品と、
出荷するプラグインとで条件が違う。

| | ライセンス | 中身 |
|---|---|---|
| `crates/nxe-audio` | **MIT OR Apache-2.0** | 共通の音声処理 |
| `crates/nxe-dsp` | **MIT OR Apache-2.0** | 共通の解析 |
| `crates/nxe-ui` | **MIT OR Apache-2.0** | 共通の Vizia ウィジェットとテーマ |
| `plugins/*/*-core` | **MIT OR Apache-2.0** | 各プラグインの DSP |
| `crates/nxe-plug-ui` | **GPL-3.0-only** | nih-plug と `nxe-ui` の結線 |
| `plugins/doubler/doubler` | **GPL-3.0-only** | 出荷するプラグイン |
| `plugins/velour/velour` | **GPL-3.0-only** | 出荷するプラグイン |
| `plugins/sparkleur/sparkleur` | **GPL-3.0-only** | 出荷するプラグイン |

境界は 1 つだけ。**`vst3-sys` をリンクするか、しないか。**

## なぜ下の 4 つが GPL なのか

`nih_plug` は `vst3-sys` に無条件で依存していて、`vst3-sys` は GPLv3:

> `vst3-sys` is licensed under the terms of the GNU GPLv3 license. This port is
> a derivative work of the original SDK, and while we do not redistribute any
> of the original source code, it was not made in isolation.

**nih-plug 本体は ISC。** GPL はここからしか来ていない。

**配布するバンドルは `.clap` も `.vst3` も GPLv3。** 3 つのプラグインは 1 つの
cdylib から `nih_export_clap!` と `nih_export_vst3!` を両方呼んでいて、
`cargo xtask bundle` はその**同じバイナリ**を両方のバンドルに入れる。だから
「CLAP 版は VST3 を含まないので緩い」は**この構成では成り立たない**。

## なぜ上の 6 つは緩くできるのか

`vst3-sys` を一切引かないから。依存を数えて確かめてある:

```text
nxe-audio    依存ゼロ
nxe-dsp      criterion（dev）のみ
nxe-ui       vizia のみ
*-core       nxe-audio と、dev の criterion / nxe-dsp のみ
```

これは**設計の帰結**で、偶然ではない。`docs/specifications/architecture.md` が
「`<plugin>-core` はホストフレームワークにも UI ツールキットにも依存しない」
「`nxe-ui` は nih-plug を知らない」を要件にしているので、GPL の伝播もそこで
止まる。

## 上流が動いた

**Steinberg は VST3 のインターフェースヘッダを MIT に再ライセンスしている**
（`vst3_pluginterfaces` の `LICENSE.txt` に `MIT License / Copyright (c) 2025,
Steinberg Media Technologies GmbH`）。`vst3-sys` が GPLv3 を選んだ理由 —
当時の SDK が proprietary / GPLv3 の二択だったこと — はもう存在しない。

ただし**`vst3-sys` 自身が GPLv3 なのは著作権者の選択**なので、SDK の
再ライセンスが遡って `vst3-sys` を緩くすることはない。外すにはリンクを
やめるしかない。

**その道は既にある。** `coupler-rs/vst3-rs` が MIT のヘッダから生成した
`vst3` クレート（MIT OR Apache-2.0）を出していて、nice-plug（ISC）がそれを
使っている。

**Air で試すのは見送った**（`AIR-4`、2026-08-27）。理由はライセンスではなく
範囲で、**移行は 1 本では終わらない**:

- ワークスペースの `vizia` は `nih_plug_vizia` が引くフォークと**バイト単位で
  一致していないとコンパイルが通らない**（Cargo にとって別 git ソースは
  別クレート）。`Cargo.toml` にその理由が書いてある
- Air だけ vizia-plug に載せると `nxe-ui` の vizia も動く。**Doubler /
  Velour / Sparkleur の UI と gallery が全部巻き添え**になり、実質 3 本の
  再検証が付いてくる
- さもなくば Air は `nxe-ui` を一切使えず、ウィジェットを自前で持つ

「駄目なら戻すのはラッパだけ」が成り立つのは **`air-core` まで**（テストごと
無傷）で、UI 層はそうではない。**やるなら UI が固まってからワークスペース
一括で**、`VST3_CLASS_ID` と状態のシリアライズ形式の一致を先に測ってから。

## 埋め込むフォント

**フォントはバイナリに焼き込まれる。** `include_bytes!` なので、`nxe-ui` を
リンクした時点でフェイスはそのプラグインの一部になる。**どれもライセンス文の
同梱を要求する**ので、リリースのバンドルにライセンス文が入っていないと条件を
満たさない。

| フェイス | ライセンス | どこ | 何に使うか |
|---|---|---|---|
| Inter（Light / Regular） | **SIL OFL 1.1** | `crates/nxe-ui/assets/inter/` | 語 |
| Geist Mono（Regular） | **SIL OFL 1.1** | `crates/nxe-ui/assets/geist/` | 数値 |
| Lucide | **ISC** | `crates/nxe-ui/assets/lucide/` | アイコン |

**OFL はフォント自体の条件で、それをリンクしたソフトウェアには伝染しない。**
`nxe-ui` が MIT OR Apache-2.0 のままでいられるのはこのため — 上の境界
（`vst3-sys` をリンクするか）とは別の話。**フォントを改変して再配布する場合は
別の条件が付く**（OFL の予約名条項）が、ここでは無改変で埋め込んでいる。

**Geist Sans は 0.2.0 で外した**（`UI-16`）。語の面が Inter に替わり、Geist は
Mono だけが残っている。ライセンス文はその Mono のために置いてある。

## 貢献するとき

**どのクレートに書くかでライセンスが決まる。** 上の 6 つに出した変更は
MIT OR Apache-2.0 の条件で受け取る。GPL のコードを上の 6 つに持ち込まない。
