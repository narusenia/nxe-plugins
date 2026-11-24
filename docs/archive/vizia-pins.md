# v0 の依存のピン — nih-plug と vizia

> v0（0.x）の `docs/specifications/architecture.md` にあった節。v2 で nih-plug と
> vizia をワークスペースから外したので、記録としてここに移した。

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

### そのフォークを、さらにフォークしている

**`[patch]` でワークスペース全体の vizia を `narusenia/vizia` の
`nxe-2026-08-27` に差し替えている。** `patched-2024-05-06` に 2 コミット
足したもので、`[patch]` はグラフ全体のソースを書き換えるので
**`nih_plug_vizia` の vizia も同じものになる** — 上の「完全に同一でなければ
ならない」はそのまま守られる。

直したのは baseview バックエンドの 2 つで、**どちらも実測で決めた**
（`mise run gallery` のアイドル時 CPU が **27.7 % → 0.9 %**）:

1. **毎フレーム無条件に全画面を再描画していた。**
   `Application::on_frame_update` が `should_redraw` を計算しているのに
   `on_frame` が読んでいない。**`vizia_winit` は同じフラグを読んでいる**ので、
   baseview 側の書き忘れ。macOS ではこのフレームタイマーが**ホストの
   run loop** に載るため、再描画がそのままホストの UI を止めていた
2. **`TextContext::sync_styles` がテキストビュー 1 個・1 フレームごとに
   `fontdb` のクエリを投げていた。** キャッシュ無し、データベースには
   システムフォント全部。描画時間の 4 分の 1

**上流に投げる価値がある。まだ投げていない。**

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

