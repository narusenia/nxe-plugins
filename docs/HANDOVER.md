# 引き継ぎ

**2026-08-27 時点。** Doubler は `doubler-v0.1.0` を**公開済み**、Velour は
`velour-v0.1.0` で**一区切り**（下書き Release）。**Sparkleur は `SPK-1`〜`SPK-14` と `SPK-16` / `SPK-17` —
残っているのは Advanced タブ（`SPK-15`）と既定値（`SPK-18`、耳が要る）だけ**。
**実機で見ていないので、寸法も含めてまだ確認が要る。**

このファイルは**そのとき何が動いていて、次に触る人が最初に知るべきこと**を
1 枚にまとめたもの。設計の正は各仕様書、状態の正は
[`implementation/backlog.md`](implementation/backlog.md) にある。

## 5 分で状況を掴む順番

1. [`../AGENTS.md`](../AGENTS.md) — リポジトリの地図と規約
2. [`implementation/backlog.md`](implementation/backlog.md) の「現在地」 — 今どこか
3. 触るプラグインの `plugins/<name>/docs/implementation/<name>-plan.md` の
   **該当単位の「決めたこと」と「踏んだ罠」** — ここに時間を溶かした記録がある
4. [`../.agents/rules/vizia.md`](../.agents/rules/vizia.md) — **UI を触るなら先に読む**

## 何ができているか

| | |
|---|---|
| `plugins/doubler` | NXE Doubler。CLAP + VST3。`doubler-v0.1.0` **公開済み** |
| `plugins/velour` | NXE Velour。CLAP + VST3。パラメータ 22 個。UI・解析・既定値まで完了。`velour-v0.1.0` |
| `crates/nxe-audio` | 共通の**処理**（`shaper` / `oversample` / `biquad` / `envelope` / `guard` / `harmonics`）。`SPK-1` で `velour-core` から抜いた |
| `crates/nxe-ui` | 共通ウィジェット・テーマ・アイコン。`mise run gallery` |
| `crates/nxe-plug-ui` | nih-plug のパラメータと `nxe-ui` の結線。**両方を知る唯一のクレート**（`SPK-11`） |
| `crates/nxe-dsp` | 共通の解析（`Handoff` / `PanScope` / `Spectrum` / `Level`） |
| `plugins/sparkleur/sparkleur-core` | **DSP は全部。** 5 帯域クロスオーバー、検波、上下コンプ、Sparkle、`CHARACTER`、De-Harsh / Sub Protect、エンジン |
| `plugins/sparkleur/sparkleur` | NXE Sparkleur。CLAP + VST3。パラメータ 33 個。**UI は 7 ノブ + タブ + Band Field + 伝達曲線の小窓 + メーター**（Advanced は `SPK-15`） |
| `plugins/sparkleur/docs` | 要件・DSP 仕様・UI 仕様・実装計画（`SPK-1`〜`SPK-18`）。`sparkleur` ラッパクレートは無い |
| CI | `check`（PR と main への push）、`release`（`<plugin>-v<version>` タグ） |

テスト 346 本。CPU は予算 533 µs に対し
**Doubler 85 µs / Velour 128 µs / Sparkleur 129 µs**
（`VEL-16`。Velour の内訳はエンジン 4x が 79、`Spectrum` 48 バンド × 2 が 45、
`Level` × 4 が 4）。

## 次にやること

**まず実機で 1 回鳴らす。** `mise run install sparkleur` して DAW で読み込む。
`SPK-8` の完了条件で唯一残っているのがこれで、**耳と DAW が要る**。UI はまだ
無いのでホストの汎用ビューで触ることになる。

次は **`SPK-15`（Advanced タブ）** — per-band の UP / DOWN / GAIN / SOLO の
表と、`FOCUS` / `DE-HARSH` / `SUB PROT` / `SNAP` / `LIFT` / `OVERSAMPLE`。
`ui.md` の表の形は Velour の Advanced とほぼ同じで、**バーには必ず高さを
指定する**（Velour はそれを忘れて表全体が「動かないコントロール」になった）。
**最後が `SPK-18`（既定値。耳が要る）。**

**`SPK-18` に持ち越した宿題が 1 つ**: `CHARACTER` の既定 0.27 は読み値が
「GLOSS 27 %」になる（一番近いアンカーが GLOSS のため）。既定を 0.25 未満に
するか、読み値の規則を変えるか。

**ここまで通っている**: `SPK-1` の `nxe-audio`（`shaper` / `oversample` /
`biquad` / `envelope` / `guard` / `harmonics`、`guard` は `RelativeGuard<N>`）、
`SPK-2` のクロスオーバー（44.1〜192 kHz・`FOCUS` 全域で和が ±0.1 dB 以内）、
`SPK-3` の検波（帯域ごとのパワー、`SPEED` を帯域中心の周期で下から抑える床）、
`SPK-4` の上下コンプ（状態を持たない純関数。`SPARK` = 0 がちょうど 0 dB）、
`SPK-6` の Sparkle（折り返し −60 dB 以下、持続音でゲート 0.008）、
`SPK-5` の `CHARACTER`（軸端から端でラウドネス 0.98 dB）、
`SPK-7` の De-Harsh（±12 dB の入力で動作量 0.2 dB 以内）と Sub Protect、
`SPK-8` のラッパ（`MIX` = 0 でビット一致、`SPARK` = 0 で ±0.1 dB 平坦）。

**この 2 つは Sparkleur のコードを 1 行も書かずに着手できる**:

| | 何 | なぜ先か |
|---|---|---|
| `SPK-10` | `nxe_ui::band::Band` の `reduction` → 符号付き `delta` | 上げコンプが半分の製品で、絵が上げを描けない |
| `SPK-11` | `param_bind` を新クレートに共通化 | Doubler と Velour で同内容が 2 つ。**3 個目が要求した** |

**設計で解いた一番大きい問題は v1 の線引き。** 概念文書は 4 製品ぶん
（OTT / Exciter / Widener / Transient）あって、`roadmap.md` 自身が「v1 の
線引き自体が難問」と書いていた。WIDTH と PUNCH を v2 に置いた理由は
`REQ-SPK-019` / `REQ-SPK-020` にある。

## 仮のまま残してある定数

**動かす理由が出てから触る。** 既定値は `VEL-17` で決めた（`DRIVE` 80 /
`BODY` 40 / `PRESENCE` 60 / `AIR` 40 / `TEXTURE` 50 / `DENSITY` 50 / `MIX` 80）。

| 何 | 聴きどころ |
|---|---|
| `TEXTURE` のトリム 9 個 | Warm → Edge で **−3.0 dB** レベルが動く残差を吸収するためにある。一番「仮」なところ |
| Guard のしきい値 −8 / −14 dB | 普通のボーカルで**動かない**のが正。動くなら厳しい |
| `EMOTION` の係数 3 つ | 0.50 / 0.40 / 0.30。声量差で質感が変わるのが**分かるが不安定でない**か |
| `REFERENCE_DB` −18 dB | `EMOTION` の軸の中心と `DENSITY` の支点を兼ねる 1 個の数。動かすと両方に効く |
| `BIAS_LEVEL_DB` 0 | 知覚的な密度をどれだけ返すか。機構は仕様、量は耳 |

## Velour で踏んだ罠（同じ形を 3 回やった）

**テスト信号の周波数を「周期数」で書くな。** 周期数はバッファ長とサンプルレートに
依存するので、間違えても落ちず「符号が逆の測定値」として出てくる。`VEL-2` /
`VEL-8` / `VEL-9` で 3 回踏んだ。**`harmonics::tone(amplitude, hz, rate, length)`
と `bin_of(hz, rate, length)` を足したので、以後は周波数で書く。**

**「往復すれば分かる」は嘘のことがある。** オーバーサンプラの位相の割り当てを
逆にしていたが、往復テストは通り低域の正弦も無事に戻ってきた。壊れていたのは
**その間に居る非線形が見る信号**で、イメージが −88 dB ではなく −24 dB だった。
`the_upsampler_leaves_no_image` がそれを固定している。

**エンベロープが絡むものはブロックごとに `set_shape` して測れ。** テストヘルパ
`rendered` は最初のサンプルの前に 1 回だけ呼ぶので、検波器はまだ無音しか
見ていない。`EMOTION` の向きが**全部逆に測れた**。ホストと同じ形で回す
`blocked` を別に用意してある。

**テストが「何も検出できないまま通る」ことがある。** `VEL-10` のジッパーの
テストは、可動域の 25% を 1 段で動かしても通っていた（エンジンにパラメータで
跳ねるゲインが 1 つも無いので、実は正しい）。**測定が失敗できることを同じテスト
の中で確かめる**行を足してある。

**引き算で「足した層」を取り出すな。** `出力 − 原音` は原音の方が大きいと層の
精度をほとんど捨てる。`SOLO` を全部オンにして層を直接取る。

## Sparkleur で踏んだ罠

**ここまで落ちたテストは 4 本とも「実装ではなく期待値」が間違っていた。**
落ちたら先に実測を並べて理論値と突き合わせるほうが速い。

**分割の裾は 24 dB/oct とは限らない**（`SPK-2`）。木構造なので各帯域は自分の
境界だけでなく**上流の全部のハイパス**を通っている。band 5 の 6 kHz より
1 オクターブ下は `HP(1500)` がまだ効いていて 30 dB/oct で落ちる。24 dB/oct を
測るなら境界が 1 つしか効かないところ。**帯域の幾何中心も 0 dB ではない**
（band 2 は 1.7 オクターブ幅で中心が −1.5 dB）。

**1 次のフィルタは 0 に到達しない**（`SPK-16`）。無音が続くと Sparkle の
ゲートが **1.3e-42** に落ち着いた。画面上は「閉じきらないゲート」。**描けない
大きさを切ったら 0 にする。** 同じ形が検波器と Guard のエネルギーにも残って
いるが、**そちらは放っておいてよい** — `SPK-17` で調べたところ
**nih-plug が `process` の前後で FTZ を x86 でも aarch64 でも有効にする**ので、
ホストの中ではデノーマルになりようがない（無音に落ちていく経路の実測も
働いているときと同じ 110 µs）。

**NaN 1 個で全部止まる場所は 1 つとは限らない**（`SPK-9`）。Velour の
`VEL-10` は検波器で見つけたので Sparkleur も検波器は守っていたが、**分割その
ものが信号経路**なのでクロスオーバーの biquad がラッチした。**入口で 1 回
消毒する**（40 個のフィルタそれぞれに番人を置くより安い）。原音は消毒しない —
通り道なので NaN を送ってきたホストには NaN が返る。

**レート非依存を定在音で測るな**（`SPK-9`）。IIR オーバーサンプラの遅延は
**サンプル数で固定**なので、層と原帯域が足し算される位相がレートで回る。
7 kHz の正弦で 48 対 96 kHz が 1.0 dB ずれて見えるが、分割だけなら 3 レートで
**完全に同じ数字**。**ノイズで測る。**

**保護は `SPARK` に乗せる**（`SPK-8`）。De-Harsh を `SPARK` と独立にすると、
**`SPARK` = 0 でも平坦にならない** — 2 kHz の単音は自分の参照帯に対して
しきい値を超えているので引かれる。`REQ-SPK-008` の
`Spark ↑ → Harshness Suppression ↑` がまさにその掛け算で、**要件を読み返したら
答えが書いてあった**。保護を足すときは「量の macro に乗るか」を先に決める。

**非対称なフォロワを比の分子に置くな**（`SPK-6`）。仕様の Sparkle は fast を
1 ms / 40 ms の非対称にしていたが、非対称な追従は平均とピークの間に座るので
（下の `SPK-3`）、対称な slow との比が**定常音でも 3 dB 出る** — ゲートが
0.44 開いたままだった。**フォロワは両方対称にして、リリースは比の後ろの
ホールドに置く。** 同じ形は「2 つの検波器の比」を取るところ全部にある。

**有限の dB は有限のゲインではない**（`SPK-4`）。hostile テストが `GAIN` に
1e9 dB を渡したとき、dB は有限のまま `linear()` が無限大を返した。
**Sparkleur でここまで唯一の「実装側の」バグ**（他はどれもテストの期待値の
間違い）。`MAX_GAIN_DB` = ±48 dB で蓋をしてある — **趣味の制限ではなく算術の
制限**なので、`CHARACTER` の値を触るときに動かすものではない。

**上げコンプの上限と床はどちらが先に効くか分からない**（`SPK-4`）。既定の床
−60 dB では POLISH も GLOSS も `CEILING` に届かず（実測 4.0 / 8.0 dB 対
上限 6 / 9 dB）、CRUSH だけが当たる。**聴き手が当たっている制限が `CHARACTER`
で入れ替わる**ので、`SPK-18` で数字を動かすときはここを見る。

**検波値は帯域の RMS ではない**（`SPK-3`）。非対称な 1 次追従は平均に落ち着かず、
**RMS の +0.05〜+2.54 dB** に座る。位置を決めるのは attack/release の比だけ
（比 4 で +0.05〜0.9、比 20 で +2.5）。素直なコンプの挙動だが、**しきい値を
計算で出せない**ということでもある — `SPK-18` を耳でやる根拠。

**定常正弦は時定数の床を測る道具として弱い**（`SPK-3`）。50 Hz では release の
20 ms が単独で読み値を持ち上げるので、attack が 1 ms でも 33 ms でもリプルは
1.0 dB のまま変わらない。**「対照実験が差を出さない」形の失敗**で、`VEL-10` の
「測定が失敗できることを同じテストで確かめる」行がそれを捕まえた。差が出るのは
**ステップ応答**（床のある帯域と無い帯域で 33 倍）。

**仕様の数字は 4 回動いた。** `k` の上限 20 → 6 → 8、`bias` のレベル補正
6 dB → 0、`DENSITY` のメイクアップの基準 full scale → `REFERENCE_DB`。
どれも実測が仕様を否定した結果で、**理由と実測値を `dsp.md` と
`velour-plan.md` に残してある**。同じ数字を触るなら先にそこを読む。

## Doubler で踏んだ罠

このリビジョンの vizia には**コンパイルも通り、エラーも出さず、何もしない**
ものがある。全部 [`../.agents/rules/vizia.md`](../.agents/rules/vizia.md) に
理由付きで書いてあるが、特に刺さりやすいのは:

- **CSS の `font-size` は単位を受け付けない。** `12px` はパース失敗で宣言ごと
  捨てられ、既定の 16 px になる
- **`cx.add_timer` は baseview で一度も発火しない。** 周期処理は `cx.spawn`
- **未指定のサイズは `Auto` ではなく `Stretch(1.0)`**
- **当たり判定を持つ子は親の押下を飲む**

**「直したつもりで直っていない」を疑う前に、まず二分探索する** —
この開発で一番時間を溶かしたのはそこ。

## 残っている宿題

| 何 | どこ | なぜ残っているか |
|---|---|---|
| `DBL-13` 既定値の詰め | `doubler-plan.md` | **耳が要る** |
| Velour の `SOLO` が保存される | `params.rs` | nih-plug に逃げ道が無い。**画面には出るようにした** — Advanced の `ON` と、`BandField` が他の区画を落とすこと（`band.rs` の `soloing`）。図はタブに関係なく常に見えるので、ラッチしたまま開いても分かる |
| 値の直接入力 | `crates/nxe-ui/src/entry.rs` | gallery では動くがプラグインに載せると editor の表示が止まる。**原因未特定** |
| `UI-9` `ToggleSwitch` | `nxe-ui-plan.md` | **2 個のプラグインがどちらも要らなかった。** 3 個目でも要らなければ落とす（Sparkleur の UI 仕様も `.segment` の `Label` で足りている） |
| 混ざったコミット `f89b40c` / `68d5199` | — | それぞれフォント修正 + Dry Gain 削除、コード + 文書の一部。分けるなら push 済み履歴の書き換えが要る |

## 次にプラグインを足すとき

`docs/specifications/architecture.md` の「新しいプラグインを足す」に手順がある。
`nxe-ui` と `nxe-dsp` はそのまま使える。

**3 個目は Sparkleur**（マルチバンドダイナミクス + Harmonic Sparkle）。
順序の根拠は [`implementation/roadmap.md`](implementation/roadmap.md) の
「Velour の後」。材料は `SPK-1` で `nxe-audio` に移してある。

**「上げるのは 2 個目が要求してから」は当たったが、境界は 2 か所ずれた**
（`SPK-1`）。**測定ヘルパの `harmonics` も一緒に動く** — 移すモジュールの
テストが全部それで書かれていて、`nxe-audio` は `velour-core` に依存できない。
**耳で決めた数は残す** — `envelope` は機構だけ移し、attack 5 ms /
release 150 ms と `REFERENCE_DB` は `velour-core` の `envelope::vocal()` に
置いた。4 個目を足すときも同じ 2 つを見る。

## リリースのしかた

```bash
git tag -a doubler-v0.1.1 -m 'NXE Doubler 0.1.1'
git push origin doubler-v0.1.1
```

タグからプラグイン名とバージョンを読み、3 OS でビルドして**下書き** Release に
zip を付ける。**バージョンはプラグインの `Cargo.toml` が持つ**ので、タグと
食い違うと CI が落ちる（わざとそうしてある）。
