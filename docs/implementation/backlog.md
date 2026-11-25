# 実装バックログ

全計画の実装単位を 1 枚に並べたもの。**着手できるものを探すためのファイル**で、
設計の正は各計画書にある。

- 単位の内容・完了条件は計画書を見る。ここには要約しか書かない
- 計画書を更新したらこの表も更新する。片方だけ直さない
- 完了した単位は行を消さず `✅` にして、PR 番号かコミット SHA を入れる
- **順序の判断は [`roadmap.md`](roadmap.md)**。この表は「何があるか」、
  ロードマップは「どの順でやるか、なぜその順か」
- v0.x の全単位は [`../archive/v0-backlog.md`](../archive/v0-backlog.md)

最終更新: 2026-11-25

## 凡例

| 記号 | 意味 |
|---|---|
| ✅ | マージ済み |
| 🟡 | 着手可能（依存が解決済み） |
| ⬜ | 依存待ち |
| ❓ | 前提条件の判断待ち（耳での確認・測定など） |
| ❌ | 判断の結果やらないことにした（根拠は計画書に記録） |

## 現在地

**v2 — truce + egui へ移し、5 本を `NXE <name> 2` として `2.0.0` から出し直す**
（2026-11-24 に計画）。決定と単位は [`v2-plan.md`](v2-plan.md)。v0.2.0 は
5 本とも出荷済み（2026-08-31）。

## 今すぐ着手できるもの

| ID | 単位 |
|---|---|
| V2-3 | 描画予算を測る |

V2-3 は V2-1 の**コード**に依存していて、V2-1 に残っている実機の確認
（下）には依存していない。

## 全単位

### v2 — [`v2-plan.md`](v2-plan.md)

| ID | 単位 | 依存 | 状態 |
|---|---|---|---|
| V2-1 | truce の骨格と Doubler 2 の起動（`au_manufacturer` / `fourcc` を 5 本分決める、`release/v0` を切る） | — | ❓ **実機が要る**。コードと validate は #7。Ableton / Studio One / Logic での保存→再読込と自動化が残り |
| V2-2 | マウス合成の修正を引き継ぐ | V2-1 | ❓ **実機が要る**。コード側は該当なし（`v2-plan.md` の V2-2）。窓を開閉したあとホストの UI がポインタに付いてくるかを見る |
| V2-3 | 描画予算を測る（1 コアの 10 %） | V2-1 | 🟡 standalone で窓の分が約 10 %（30 Hz、`investigations/v2-frame-cost.md`）。音を通したホストでの測定が残り（人の手） |
| V2-4 | 窓の回帰を CI で検出する（`cargo truce screenshot --check`） | V2-1 | ✅ #10。macOS / Windows の基準画像、1 px ずらして両 OS で落ちることを確認（3174d9b） |
| V2-5 | `nxe-ui` の egui 版とギャラリー | V2-2, V2-3 | ✅ `crates/nxe-egui`。全部品がギャラリーにあり、パレットのテストを引き継いだ |
| V2-6 | 信号表示の平滑化 | V2-5 | ✅ `nxe_egui::smooth`。30 Hz の読みを 60 Hz で描いて毎フレーム動くことをテストで確認 |
| V2-7 | Doubler 2 を型に載せる | V2-5, V2-6 | 🟡 コードと基準画像（macOS / Windows、CI で焼いたもの）は済み。残りは実機で見ること、描画予算の測定、付属プリセットの中身を耳で決めること |
| V2-8 | Basic の beta（6 本まとめて、`basic-<YYYY.MM>-beta.N`） | V2-4, V2-7, V2-13 | ✅ `basic-2026.11-beta.1`（2026-11-25、プリセットと Pumice の PDC は未確認のまま β として出した） |
| V2-9 | Sparkleur 2（型を確定する） | V2-7 | 🟡 コード、寸法の書き戻し、基準画像（macOS / Windows、CI で焼いたもの）は済み。残りは実機で見ること、付属プリセットの中身を耳で決めること |
| V2-10 | Velour 2 | V2-9 | 🟡 コード、基準画像（CI で焼いたもの）は済み（#20）。残りは実機で見ること、付属プリセットの中身を耳で決めること |
| V2-11 | Air 2 | V2-9 | 🟡 コード、基準画像（CI で焼いたもの）は済み（#21）。残りは実機で見ること、付属プリセットの中身を耳で決めること |
| V2-12 | Diorama 2 | V2-9 | 🟡 コード、基準画像（CI で焼いたもの）は済み（#22）。残りは実機で見ること、付属プリセットの中身を耳で決めること |
| V2-13 | リリースの CI（beta の前） | V2-1 | ✅ #11。PR の試運転で macOS（universal）/ Windows の zip と validate を確認。draft release を実際に作るのは V2-8 のタグが最初 |
| V2-15 | Pumice を main に入れる（nih-plug のまま） | — | ✅ #9 |
| V2-16 | Pumice 1.0（truce、`NXE Pumice` / `Pumi`） | V2-9, V2-15 | 🟡 コード、基準画像（CI で焼いたもの）は済み（#26）。残りは 3 ホストでの反転試験、付属プリセットの中身を耳で決めること |
| V2-17 | nih-plug と vizia の側を消す（`nxe-ui`、`nxe-plug-ui`、xtask、`bundler.toml`、フォークへの patch） | V2-16 | ✅ |
| V2-14 | 5 本の 2.0.0 と Pumice 1.0.0 | V2-8, V2-10〜V2-13, V2-16 | ⬜ |

### Peek — [`peek-plan.md`](../../plugins/peek/docs/implementation/peek-plan.md)

| ID | 単位 | 依存 | 状態 |
|---|---|---|---|
| PEEK-1 | Kit の窓の型（`nxe-egui::kit`）、`peek-core`、Peek Level | — | 🟡 コード、validate、基準画像は済み（#35）。止まったら無音を描く（#37）。実機で見ることが残り |
| PEEK-2 | Peek VU（弧と針の見た目を含む） | PEEK-1 | 🟡 コードは済み（#35）。実機で見ることが残り |
| PEEK-3 | Peek Stereo | PEEK-1 | 🟡 同上 |
| PEEK-4 | Peek Spectrum | PEEK-1 | 🟡 同上 |
| PEEK-5 | Peek Spectrogram | PEEK-4 | 🟡 同上。`nxe-spectral` への切り出しは後回し（`peek-plan.md`） |
| PEEK-6 | Peek Loudness | PEEK-1 | 🟡 同上。EBU の試験条件 3 つは試験で確認済み |
| PEEK-7 | Peek Scope | PEEK-1 | 🟡 同上 |
| PEEK-8 | Peek（まとめ、ホストでリサイズ可） | PEEK-2〜7 | 🟡 同上 |
| PEEK-9 | 最初の beta（`peek-<YYYY.MM>-beta.N`） | PEEK-1 | ✅ `peek-2026.11-beta.1`（2026-11-25、1cdbb9a） |

### Kit — [`nonlinear-kit-plan.md`](../../plugins/kit/docs/implementation/nonlinear-kit-plan.md)

| ID | 単位 | 依存 | 状態 |
|---|---|---|---|
| KIT-1 | `Palette::DESIGN_KIT`、`kit-ui`、`nonlinear-core`、Fold | — | 🟡 コード（#36）と基準画像（macOS / Windows、#38）は済み。付属プリセットと既定値を耳で決めることが残り |
| KIT-2 | Fever | KIT-1 | 🟡 同上 |
| KIT-3 | Buckle | KIT-1 | 🟡 同上 |
| KIT-4 | 最初の beta（`kit-<YYYY.MM>-beta.N`） | KIT-1〜3 | ✅ `kit-2026.11-beta.1`（2026-11-25、1cdbb9a）。Spectral Kit の 3 本も同じ版に乗った |
| KIT-5 | `nxe-spectral` の切り出し、`spectral-core`、Invert（[`spectral-kit-plan.md`](../../plugins/kit/docs/implementation/spectral-kit-plan.md)） | KIT-1 | 🟡 コード（#37）と基準画像（#38）は済み。耳が残り |
| KIT-6 | Shear | KIT-5 | 🟡 同上 |
| KIT-7 | Afterimage | KIT-5 | 🟡 同上 |
| RES-1 | `nxe-audio::pitch`、`Palette::COLOUR_KIT`、`resonance-core`、Dew（[`resonance-kit-plan.md`](../../plugins/kit/docs/implementation/resonance-kit-plan.md)） | — | ✅ コードと基準画像（macOS / Windows）は済み。2026-11-25 に耳で確かめた（Sensitivity、Pitch、Follow を直した）。付属プリセットの値は仮 |
| RES-2 | Halo | RES-1 | 🟡 コードと基準画像（macOS / Windows）は済み。耳が残り |
| PRT-1 | `nxe-audio::onset`、`particle-core`、Glint（[`particle-kit-plan.md`](../../plugins/kit/docs/implementation/particle-kit-plan.md)） | — | 🟡 コードと基準画像（macOS / Windows）は済み。耳が残り |
| PRT-2 | Pollen | PRT-1 | ✅ 付属プリセットは 2026-11-26 に耳で確かめた。コードと基準画像（macOS / Windows） |

### Design — [`parasite-plan.md`](../../plugins/parasite/docs/implementation/parasite-plan.md)

| ID | 単位 | 依存 | 状態 |
|---|---|---|---|
| PAR-1 | `parasite-core`: ピッチ追従（YIN）とエンジン、4 種 | — | ✅ 2026-11-25 に付属プリセットで耳で確かめた |
| PAR-2 | `nxe_egui::design`（Design 7 の枠と細いスライダー） | — | ✅ |
| PAR-3 | Parasite のプラグイン | PAR-1, PAR-2 | ✅ |
| PAR-4 | 最初の beta（`design-<YYYY.MM>-beta.N`） | PAR-3 | ✅ `design-2026.11-beta.1`（2026-11-25、Latest） |

### v0 から持ち越したもの

単位の内容は各プラグインの計画書が正。DSP は `<plugin>-core` にあるので、
v2 の移行とは独立に進められる。

| ID | 単位 | 計画 | 状態 |
|---|---|---|---|
| DBL-13 | 既定値の詰めと実機確認 | [`doubler-plan.md`](../../plugins/doubler/docs/implementation/doubler-plan.md) | ❓ **耳が要る** |
| DIO-13 | 既定値と耳 | [`diorama-plan.md`](../../plugins/diorama/docs/implementation/diorama-plan.md) | ❓ **耳が要る** |
| — | Diorama の Advanced の偏差（`REQ-DIO-009`）。前に `dsp.md` が各偏差の数を書く | [`diorama-plan.md`](../../plugins/diorama/docs/implementation/diorama-plan.md) | ⬜ |
| PUM-11 | Pumice の既定値と耳。`DEPTH` という名前を使えるかもここで決着（`REQ-PUM-010`） | [`pumice-plan.md`](../../plugins/pumice/docs/implementation/pumice-plan.md) | ❓ **耳が要る** |
| SPK-18 | Sparkleur の主観サインオフ | [`sparkleur-plan.md`](../../plugins/sparkleur/docs/implementation/sparkleur-plan.md) | ❓ **耳が要る** |
| INFRA-3 | 署名と notarize（リリースの CI のうち未了の部分） | [`infra-plan.md`](infra-plan.md) | ⬜ |

### まだ単位にしていないもの

| 何 | 状態 |
|---|---|
| Vocal Glue | 要件まで（[`REQ-GLU.md`](../../plugins/vocal-glue/docs/requirements/REQ-GLU.md)） |
| Colour シリーズ / Design シリーズ | 構想のみ。型の候補は [`design-system.md`](../specifications/design-system.md) |
| AU v2 の名前が `NXE: NXE Doubler 2` と二重になる | cargo-truce 6.3.0 の AU v2 の Info.plist は `au_name` を読まず `name` を使う。rescan と一緒に上流へ |
| truce-egui の 2 か所を上流へ送る | `patches/truce-egui` の `with_font_definitions` と、遅延付きの再描画要求を「今すぐ」と扱う判定の修正。上流に入れば patch を消せる。rescan と一緒に、出す前に確認 |
| truce-clap の rescan を上流へ送る | `patches/truce-clap` の 1 か所。上流に入れば patch を消せる。外への PR なので出す前に確認する |
| パワーフォロワの共通化 | 3 個目が来ているが形が 3 つとも違う。上げるなら何を共通にするかを先に決める |
