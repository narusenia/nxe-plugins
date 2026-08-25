# インフラ実装計画

> 最終更新: 2026-08-25

ワークスペースの骨格と CI。設計の正は
[`../specifications/architecture.md`](../specifications/architecture.md)。
状態は [`backlog.md`](backlog.md)、順序の根拠は [`roadmap.md`](roadmap.md)。

## INFRA-1 — ワークスペース骨格

ルートの `Cargo.toml`（workspace）、`xtask`（`nih_plug_xtask`）、
`crates/nxe-ui`、`plugins/doubler/doubler-core`、`plugins/doubler/doubler`、
`plugins/doubler/bundler.toml` を空の状態で作る。プラグインは音を出さないが
バンドルはできる状態にする。

- **完了条件**: `mise run check` が通る。`mise run bundle doubler` が
  `target/bundled/` に `.clap` と `.vst3` を出す。`mise run gallery` が
  ウィンドウを開く。依存の向きが `architecture.md` のとおりで、
  `doubler-core` の依存に nih-plug も Vizia も無い
- **依存**: なし

## INFRA-2 — プルリクエストの CI

GitHub Actions で `mise run check`（fmt + clippy + test）。macOS で回す。

- **完了条件**: プルリクエストで自動実行され、失敗が分かる。ローカルの
  `mise run check` と同じことをしている（片方だけ通る状態を作らない）
- **依存**: INFRA-1

## INFRA-3 — リリースの CI

タグを打つと macOS（universal）・Windows・Linux でバンドルし、プラットフォーム
ごとの zip を GitHub Release に添付する。

- **完了条件**: タグから 3 つの zip が Release に付く。macOS の zip が
  universal（Apple Silicon + Intel）である。README の Gatekeeper 回避手順が
  実際に添付物に対して有効
- **署名は範囲外**: Apple Developer Program を契約するまでは未署名で出す。
  契約したらこの単位に署名と公証を足す（README の手順はそのとき削る）
- **依存**: INFRA-2
