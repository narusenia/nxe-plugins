# 実装バックログ

全計画の実装単位を 1 枚に並べたもの。**着手できるものを探すためのファイル**で、
設計の正は各計画書にある。

- 単位の内容・完了条件は計画書を見る。ここには要約しか書かない
- 計画書を更新したらこの表も更新する。片方だけ直さない
- 完了した単位は行を消さず `✅` にして、PR 番号かコミット SHA を入れる
  （main に直接コミットしている間は SHA）
- **順序の判断は [`roadmap.md`](roadmap.md)**。この表は「何があるか」、
  ロードマップは「どの順でやるか、なぜその順か」

最終更新: 2026-08-25

## 凡例

| 記号 | 意味 |
|---|---|
| ✅ | マージ済み |
| 🟡 | 着手可能（依存が解決済み） |
| ⬜ | 依存待ち |
| ❓ | 前提条件の判断待ち（耳での確認・測定など） |
| ❌ | 判断の結果やらないことにした（根拠は計画書に記録） |

## 今すぐ着手できるもの

依存が無いか、依存がすべて解決している単位。

| ID | 単位 | 計画 |
|---|---|---|
| UI-10 | `CurveView` | `nxe-ui-plan.md` |
| UI-8 / UI-9 | `Meter` / `ToggleSwitch`（Doubler は使わない。フェーズ 5） | `nxe-ui-plan.md` |
| INFRA-2 | プルリクエストの CI（フェーズ 4） | `infra-plan.md` |

順序は [`roadmap.md`](roadmap.md) が決める。フェーズ 3 進行中で、`UI-5` /
`UI-6` / `UI-7` / `UI-10` は互いに独立なのでどの順でもよい。これらが揃うと
`DBL-9` 以降のプラグイン側 UI に入れる。

`UI-3` はツールチップを残している（`✅ ❓`）。vizia に `Tooltip` view はあるので
最初に必要になった単位で入れる。

## 全単位

### インフラ — `infra-plan.md`

| ID | 単位 | 状態 |
|---|---|---|
| INFRA-1 | ワークスペース骨格 | ✅ `3f98dce` |
| INFRA-2 | プルリクエストの CI（`mise run check`） | 🟡 |
| INFRA-3 | リリースの CI（タグ → 3 OS bundle → Release） | ⬜ INFRA-2 |

### 共通 UI — `nxe-ui-plan.md`

| ID | 単位 | 状態 |
|---|---|---|
| UI-1 | テーマトークンと gallery の中身 | ✅ |
| UI-2 | Lucide アイコン埋め込みと定数生成 | ✅ |
| UI-3 | 共通の入力ふるまい（ドラッグ / 微調整 / リセット / 値入力 / ジェスチャー通知） | ✅ ❓ ツールチップ未 |
| UI-4 | `Knob`（大小 2 サイズ） | ✅ |
| UI-5 | `Bar` | ✅ |
| UI-6 | `SegmentedControl` | ✅ ❓ キーボード未 |
| UI-7 | `PolarField`（領域知識を持たない極座標フィールド） | ✅ |
| UI-10 | `CurveView`（領域知識を持たない対数軸の曲線表示） | 🟡 |
| UI-8 | `Meter` — **Doubler は使わない** | 🟡 |
| UI-9 | `ToggleSwitch` — **Doubler は使わない** | 🟡 |

### Doubler — `../../plugins/doubler/docs/implementation/doubler-plan.md`

| ID | 単位 | 状態 |
|---|---|---|
| DBL-1 | リングバッファ遅延線と Hermite 補間 | ✅ `a84779d` |
| DBL-2 | 回転タップピッチシフタ（**ゲート通過**） | ✅ `f2501d6` |
| DBL-3 | ボイスエンジン（形状表・実効値・パン則・ゲイン補償） | ✅ `3aec7af` |
| DBL-4 | nih-plug ラッパと配線（**ここで音が出る**。UI 無し） | ✅ `556c5a0` |
| DBL-5 | Humanize | ✅ |
| DBL-6 | Source モード（Mono Sum / True Stereo） | ✅ |
| DBL-7 | Tone と Tone Spread | ✅ |
| DBL-8 | スムージングとサンプルレート／ブロックサイズ非依存の詰め | ✅ |
| DBL-9 | UI マクロ層（ノブとセグメント） | ⬜ DBL-4, UI-4, UI-6, UI-2 |
| DBL-10 | UI Voice Field | ⬜ DBL-9, DBL-5, DBL-6, UI-7 |
| DBL-11 | UI Detail 層（ボイス表） | ⬜ DBL-10, UI-5 |
| DBL-14 | UI Filter View | ⬜ DBL-9, UI-10 |
| DBL-15 | UI ミラー編集（`REQ-DBL-014`） | ⬜ DBL-10, DBL-11 |
| DBL-12 | CPU 予算の確認（criterion） | ⬜ DBL-8 |
| DBL-13 | 既定値の詰めと実機確認 | ⬜ DBL-12, DBL-11 |
