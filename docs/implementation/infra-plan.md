# インフラ実装計画

> 最終更新: 2026-08-25

ワークスペースの骨格と CI。設計の正は
[`../specifications/architecture.md`](../specifications/architecture.md)。
状態は [`backlog.md`](backlog.md)、順序の根拠は [`roadmap.md`](roadmap.md)。

## INFRA-1 — ワークスペース骨格 ✅ `3f98dce`

ルートの `Cargo.toml`（workspace）、`xtask`（`nih_plug_xtask`）、
`crates/nxe-ui`、`plugins/doubler/doubler-core`、`plugins/doubler/doubler`、
`plugins/doubler/bundler.toml` を空の状態で作る。プラグインは音を出さないが
バンドルはできる状態にする。

- **完了条件**: `mise run check` が通る。`mise run bundle doubler` が
  `target/bundled/` に `.clap` と `.vst3` を出す。`mise run gallery` が
  ウィンドウを開く。依存の向きが `architecture.md` のとおりで、
  `doubler-core` の依存に nih-plug も Vizia も無い
- **依存**: なし
- **やってみて分かったこと**（`architecture.md` に反映済み）:
  1. **`bundler.toml` はワークスペース直下に 1 つ。** バンドラがそこを読むので
     プラグインごとには置けない
  2. **vizia の `winit` と `baseview` は相互排他。** 両方有効にすると
     `Application` がどちらも re-export されず、`nih_plug_vizia` 自身が
     コンパイルできない。dev-dependency に隔離する手も効かない（resolver v2 は
     dev ターゲットを含むビルドで feature を統合する）。**ワークスペース全体を
     `baseview` に寄せ、gallery も baseview の単体ウィンドウで開く**
  3. mise の `{{arg(name="plugin")}}` は期待どおり動く。ただしクローン後に
     `mise trust` が必要

## INFRA-2 — プルリクエストの CI

GitHub Actions で `mise run check`（fmt + clippy + test）。macOS で回す。

- **完了条件**: プルリクエストで自動実行され、失敗が分かる。ローカルの
  `mise run check` と同じことをしている（片方だけ通る状態を作らない）
- **依存**: INFRA-1
- **決めたこと**: ワークフローは `cargo fmt`／`clippy`／`test` を自分で書かず、
  **`mise run check` を呼ぶだけ**にする。書き下すと CI とローカルが別々に
  ずれていく
- **決めたこと**: `pull_request` だけでなく **`main` への push でも回す**。
  このリポジトリは main に直接コミットしているので、PR 限定だとほとんど動かない
- **決めたこと**: Rust のバージョンはワークフローに書かず `mise.toml` から取る
  （`jdx/mise-action`）。バージョンの二重管理をしない
- **決めたこと**: macOS のみ。開発とバンドルをする場所で、UI スタックが
  一番プラットフォーム差で壊れる。他の 2 つは `INFRA-3` がビルドする

## INFRA-3 — リリースの CI

タグを打つと macOS（universal）・Windows・Linux でバンドルし、プラットフォーム
ごとの zip を GitHub Release に添付する。

- **完了条件**: タグから 3 つの zip が Release に付く。macOS の zip が
  universal（Apple Silicon + Intel）である。README の Gatekeeper 回避手順が
  実際に添付物に対して有効
- **署名は範囲外**: Apple Developer Program を契約するまでは未署名で出す。
  契約したらこの単位に署名と公証を足す（README の手順はそのとき削る）
- **依存**: INFRA-2
- **決めたこと**: 各プラットフォームは**自分のランナーでビルドする**。
  ネイティブの UI スタックをリンクするのでクロスコンパイルしない。macOS の
  2 アーキテクチャだけは `cargo xtask bundle-universal` が lipo でまとめる
- **決めたこと**: Release は**下書きで作る**。macOS が未署名なので、
  リリースノートを見て人が公開ボタンを押す
- **決めたこと**: `fail-fast: false`。1 つ落ちたときに残り 2 つの成果物を
  捨てると、原因の当たりを付けにくい
- **決めたこと**: zip は Windows だけ `Compress-Archive`、他は `zip`。
  `zip` は Windows ランナーに無く、`Compress-Archive` は他に無い
- **未検証**: タグを打っていないので、このワークフローはまだ一度も走っていない。
  最初のタグで確認する項目 — 3 つの zip が付くか、macOS の bundle が
  `lipo -info` で universal か、README の Gatekeeper 手順が実際の添付物に効くか
