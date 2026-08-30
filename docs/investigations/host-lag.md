# 未解決 — UI を一度開くとホストが「スーッと」遅れる

**2026-08-31 時点。原因未特定。** いつでもここから再開できるように、
分かっていること・**外した仮説**・次に測るものを全部置いてある。

窓が開いている間の CPU コストは**別件で、そちらは片付いている**
（[`ui-frame-cost.md`](ui-frame-cost.md)、gallery で 22.6 % → 6.0 %）。
それを全部やっても**この症状は変わらなかった**、というのがこの文書の要点。

---

## 1. 症状

- プラグインの UI を**一度でも開くと**、以降ホストの操作がポインタに追従せず、
  遅れて滑るように動く（ユーザーの言葉で「瞬時じゃなくてスーッとだんだん
  移動する」）
- **プラグインを消しても直らない。ホストを再起動すると直る**
- **UI を一度も開かなければ起きない**
- **他社の VST では起きない**
- Studio One（Fender Studio Pro 8.0.3）で顕著、Ableton Live 12 でも軽く出る

## 2. 測って分かっていること

**ホストは暇。** 遅い状態で UI を閉じたまま `sample` を撮ると:

```
MAIN 3970 samples, idle 3837 (97%), busy 133 (3%)
私たちのコードのフレーム: 0
```

**WindowServer も暇で、正常時と区別がつかない。** 遅い状態と正常な状態で
`sudo sample $(pgrep -x WindowServer) 5` を撮って比べた:

| | busy サンプル |
|---|---|
| 正常 | 15 519 |
| 遅い | 15 710 |

支配的な葉は両方とも `semaphore_timedwait_trap`（待ち）。`CA::Render` の量にも
差が無い。**合成側は無罪。**

**ホストは `CAMetalLayer` で自分の UI を描いている**（スタックに
`-[CAMetalLayer nextDrawable]` / `-[CAMetalDrawable present]` が出る）。

**誰も忙しくないのに遅い。** 処理ではなく「いつ更新されるか」の問題に見える。

## 3. 外した仮説（全部、実機で確認して外れた）

| 仮説 | やったこと | 結果 |
|---|---|---|
| フレームのコストが高い | 原因 8 個を修正、CPU を 1/4 に | **症状は変わらない** |
| swap がホストのラン ループを止めている | シングルバッファ化 | 数字は最高。**ホストで窓が真っ黒**。撤回 |
| 毎フレームの `setNeedsDisplay:` がホストの display cycle を誘発 | 削除 | **悪化**。撤回 |
| `NSOpenGLView` にレイヤーが無いのが悪い | `setWantsLayer:` | 変化なし |
| **ホストの窓の中の `NSOpenGLView` が窓の合成経路を壊す** | `IOSurface` + 通常の `CALayer` に作り替え、**OpenGL のビューをホストの階層から完全に排除** | **症状は残った** |
| 合成側（WindowServer）が詰まっている | 上の測定 | **正常時と同一** |
| マルチディスプレイ／リフレッシュレート混在（120 Hz + 144 Hz） | ディスプレイを 1 台にして再現 | **変わらない** |

**5 回外している。** 次は測ってから動くこと。

## 4. 仕掛けてあるもの — `NXE_BASEVIEW_NO_PRESENT`

次の切り分けのためのスイッチが `narusenia/baseview` `nxe-2026-08-31` に入って
いる。**設定すると、レイヤーをホストの窓に足さない** — GL コンテキストは作られ、
サーフェスも確保され、毎フレーム描画とコピーも走るが、**画面には何も出ない**。

読み方:

- **これでも遅くなる** → 画面に出すことは無関係。**ホストのプロセスで OpenGL を
  初期化すること自体**（CGL コンテキスト、ドライバのロード、
  `AppleMetalOpenGLRenderer` の起動）が疑わしい。次はコンテキストを一切作らない
  ビルドへ
- **遅くならない** → 原因は「ホストの窓に何かを提示すること」。レイヤーの属性
  （`contentsScale`、更新頻度、`CAMetalLayer` との同居）を疑う番

**環境変数を DAW に渡す方法**（Finder から起動したアプリは shell の環境を
継がない）:

```bash
# Studio One を落としてから
launchctl setenv NXE_BASEVIEW_NO_PRESENT 1
# → Studio One を起動してテスト

# 戻す
launchctl unsetenv NXE_BASEVIEW_NO_PRESENT
```

あるいはターミナルから直接起動する:

```bash
NXE_BASEVIEW_NO_PRESENT=1 "/Applications/Studio Pro 8.app/Contents/MacOS/Studio Pro"
```

窓は真っ黒（何も出ない）のが正常な動作。**見るのは「DAW が遅くなるか」だけ。**

## 5. その次に測るもの

1. **コンテキストを一切作らないビルド。** `vizia_baseview` が GL 前提なので、
   `create_vizia_editor` を使わず、**空の `NSView` を返すだけの `Editor` 実装**を
   Velour に一時的に差すのが早い。これで遅くなるなら、GUI とは無関係
2. **他の OpenGL プラグインで再現するか。** `~/Library/Audio/Plug-Ins/VST3/` の
   `gliff` / `scintillate` が baseview か nih-plug 系なら、同じことが起きるはず。
   起きるなら**私たちのコードの問題ではない**
3. **ホストの更新レートを直接見る。** 遅い状態と正常な状態で、ホストのウィンドウ
   が 1 秒に何回更新されているかを数える（画面収録して数える、でもよい）。
   「滑らかだが遅れる」が本当に低フレームレートなのか、それとも入力から表示
   までの遅延なのかで、次に見る場所が変わる

## 6. 再開の手順

```bash
# 安全な現行ビルド（IOSurface 版、絵は正常）
mise run install velour          # ホストは落としてから

# 切り分けビルドは同じもの。環境変数で挙動が変わる（上の 4 節）
```

フォークの現在地:

- [`narusenia/vizia`](https://github.com/narusenia/vizia) `nxe-2026-08-30g`
- [`narusenia/baseview`](https://github.com/narusenia/baseview) `nxe-2026-08-31`

**この症状を追うときは、`ui-frame-cost.md` の対策と混ぜないこと。** あちらは
「窓が開いている間の CPU」で、実測に基づいていて、もう終わっている。こちらは
別の問題で、**まだ何も分かっていない**。
