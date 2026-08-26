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

## 現在地

**フェーズ 3（UI）はほぼ完了。** `nxe-ui` のウィジェットは Doubler が使うものが
全部揃い、プラグイン側も Voice Field・Filter View・Detail 表まで載っている。

最後に確認待ちなのは**タブ式レイアウトの見た目**（2026-08-26 時点）。窓 620 × 572、
Voice Field と `MIX` / `OUTPUT` が常時表示、タブが「4 ノブ + Filter View」と
「8 行の表」を入れ替える。寸法は `plugins/doubler/doubler/src/ui/mod.rs` の
`FIELD_HEIGHT` / `SIDE_WIDTH` / `TAB_HEIGHT` で調整できる。

## 今すぐ着手できるもの

依存が無いか、依存がすべて解決している単位。

| ID | 単位 | 計画 |
|---|---|---|
| DBL-15 | UI ミラー編集（フェーズ 3 の最後） | `doubler-plan.md` |
| INFRA-2 | プルリクエストの CI（フェーズ 4） | `infra-plan.md` |
| INFRA-3 | リリースの CI（フェーズ 4） | `infra-plan.md` |
| DBL-12 | CPU 予算の確認（フェーズ 4） | `doubler-plan.md` |
| DBL-13 | 既定値の詰めと実機確認（フェーズ 4） | `doubler-plan.md` |
| UI-8 / UI-9 | `Meter` / `ToggleSwitch`（Doubler は使わない。フェーズ 5） | `nxe-ui-plan.md` |

## 積み残し（どれも単位を持っていない小物）

`✅ ❓` が付いている単位の未完了部分。まとめて 1 単位にしてもよい。

| 何 | どこ |
|---|---|
| ツールチップ | `UI-3`。vizia に `Tooltip` view はある |
| `SegmentedControl` のキーボード左右移動 | `UI-6` |
| 値の直接入力（`Gesture::Edit` が無反応） | `UI-3`。インラインのテキスト入力が要る |
| Detail 表 → Voice Field のハイライト | `DBL-11`。`PolarField` に外からハイライトを指定する入力が無い |
| 見た目の最終調整（フォントサイズ、寸法、余白） | ユーザーの指示で最後にまとめる |

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
| UI-10 | `CurveView`（領域知識を持たない曲線表示） | ✅ |
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
| DBL-9 | UI マクロ層（ノブとセグメント） | ✅ |
| DBL-10 | UI Voice Field | ✅ |
| DBL-11 | UI Detail 層（ボイス表） | ✅ ❓ 表→図のハイライト未 |
| DBL-14 | UI Filter View | ✅ |
| DBL-15 | UI ミラー編集（`REQ-DBL-014`） | 🟡 |
| DBL-12 | CPU 予算の確認（criterion） | ⬜ DBL-8 |
| DBL-13 | 既定値の詰めと実機確認 | ⬜ DBL-12, DBL-11 |
