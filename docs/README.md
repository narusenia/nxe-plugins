# nxe-plugins ドキュメント索引

`nxe-plugins` は Rust の音声プラグインのモノレポ。各プラグインは nih-plug 上に
建て、CLAP と VST3 を出力する。UI は Vizia。

リポジトリ全体の地図と規約は [`../AGENTS.md`](../AGENTS.md)。

## 役割で引く

| 知りたいこと | 場所 |
|---|---|
| **クレート構成・依存の向き・ビルドとリリース** | [specifications/architecture.md](specifications/architecture.md) |
| **新しいプラグインを足す手順** | [specifications/architecture.md](specifications/architecture.md) の「新しいプラグインを足す」 |
| **今どの実装単位があって、どれが着手できるか** | [implementation/backlog.md](implementation/backlog.md) |
| **引き継ぎ（今どこで、次に何を知るべきか）** | [HANDOVER.md](HANDOVER.md) |
| **UI を開くと DAW が重くなる問題**（原因 2 つ、対策、外した仮説、測り方） | [investigations/ui-frame-cost.md](investigations/ui-frame-cost.md) |
| **共通 UI コンポーネントの計画** | [implementation/nxe-ui-plan.md](implementation/nxe-ui-plan.md) |
| **共通の解析（レベル・ステレオ像・スペクトラム）の計画** | [implementation/nxe-dsp-plan.md](implementation/nxe-dsp-plan.md) |
| **ワークスペース骨格と CI の計画** | [implementation/infra-plan.md](implementation/infra-plan.md) |
| **どの順でやるか、なぜその順か** | [implementation/roadmap.md](implementation/roadmap.md) |
| **まだ書かれていないプラグインの構想** | [`../concepts/`](../concepts/) |
| **共通ウィジェットの使い方**（契約・トークン・アイコン） | [`../crates/nxe-ui/README.md`](../crates/nxe-ui/README.md) |
| **守るべき規約**（Rust / UI / 文書） | [`../.agents/rules/`](../.agents/rules/) |
| **コミットとブランチの規約** | [`../.agents/rules/git.md`](../.agents/rules/git.md) |
| **画面の規則**（グリッド、色、読み値、何を出すか） | [`../.agents/rules/ui.md`](../.agents/rules/ui.md) |
| **踏んだ罠**（vizia の挙動、検証の抜け） | [`../.agents/rules/vizia.md`](../.agents/rules/vizia.md) / [`../.agents/rules/rust.md`](../.agents/rules/rust.md) |
| **プラグイン固有の要件・仕様・計画** | `plugins/<name>/docs/` |

同じ内容を 2 箇所に書かない。実装と食い違うときは**実装が正**で、気づいた
文書をその変更で直す。

## モノレポ共通か、プラグイン固有か

境界は 1 つだけ。**プラグインをこのリポジトリから切り出したときに一緒に
ついていくべき文書は、そのプラグインの下に置く。**

- ここ（`docs/`）に置くもの — クレート構成、依存の規則、ビルドと配布、
  複数プラグインをまたぐ実装の順序、複数プラグインに共通の調査
  （`investigations/`）
- プラグインの下に置くもの — そのプラグインの要件・DSP 仕様・UI 仕様・実装計画

例外は `implementation/backlog.md` と `roadmap.md`。プラグイン固有の実装単位も
ここに載せる。着手できるものを探すときに横断で 1 枚見たいからで、単位の内容と
完了条件はプラグイン側の計画書が正。

## プラグイン

| プラグイン | 内容 | 文書 |
|---|---|---|
| Doubler | マルチボイスダブラー（2/4/8 ボイス） | [要件](../plugins/doubler/docs/requirements/REQ-DBL.md) / [DSP 仕様](../plugins/doubler/docs/specifications/dsp.md) / [UI 仕様](../plugins/doubler/docs/specifications/ui.md) / [計画](../plugins/doubler/docs/implementation/doubler-plan.md) |
| Velour | ボーカルの存在感を生成するサチュレータ（並列 3 帯域） | [要件](../plugins/velour/docs/requirements/REQ-VEL.md) / [DSP 仕様](../plugins/velour/docs/specifications/dsp.md) / [UI 仕様](../plugins/velour/docs/specifications/ui.md) / [計画](../plugins/velour/docs/implementation/velour-plan.md) |
| Sparkleur | マルチバンドダイナミクス + 動的な倍音生成（分割 5 帯域） | [要件](../plugins/sparkleur/docs/requirements/REQ-SPK.md) / [DSP 仕様](../plugins/sparkleur/docs/specifications/dsp.md) / [UI 仕様](../plugins/sparkleur/docs/specifications/ui.md) / [計画](../plugins/sparkleur/docs/implementation/sparkleur-plan.md) |
| Air | 信号駆動の高域テクスチャ生成器（加算 2 系統） | [要件](../plugins/air/docs/requirements/REQ-AIR.md) / [DSP 仕様](../plugins/air/docs/specifications/dsp.md) / [UI 仕様](../plugins/air/docs/specifications/ui.md) / [計画](../plugins/air/docs/implementation/air-plan.md) |
| Diorama | 声の前後位置を作る（初期反射 + 直接音）**`DIO-12` まで実装済み** | [要件](../plugins/diorama/docs/requirements/REQ-DIO.md) / [DSP](../plugins/diorama/docs/specifications/dsp.md) / [UI](../plugins/diorama/docs/specifications/ui.md) / [計画](../plugins/diorama/docs/implementation/diorama-plan.md) |
| Vocal Glue | ボーカルバスの結束（直列 5 段）**要件のみ** | [要件](../plugins/vocal-glue/docs/requirements/REQ-GLU.md) |

## 構想

まだ要件を書いていないものは [`../concepts/`](../concepts/) に置く。**構想は
仕様ではない** — 着手が決まった時点で `plugins/<name>/docs/requirements/` に
要件として書き直し、構想側はそのまま残す（何を削ったかが後で効く）。

着手の順序とその根拠は
[implementation/roadmap.md](implementation/roadmap.md) の
「Sparkleur の後 — 構想 6 本の順序」。
