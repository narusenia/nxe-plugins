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
| DBL-6 | Source モード（Mono Sum / True Stereo） | `doubler-plan.md` |
| DBL-7 | Tone と Tone Spread | `doubler-plan.md` |
| UI-1 | テーマトークンと gallery の中身 | `nxe-ui-plan.md` |
| INFRA-2 | プルリクエストの CI | `infra-plan.md` |

順序は [`roadmap.md`](roadmap.md) が決める。今は `DBL-4`（フェーズ 1 の最後）が
次。`DBL-5`〜`DBL-7` も依存は解けているが、フェーズ 2 なので `DBL-4` の後。
`UI-1` はフェーズ 3、`INFRA-2` はフェーズ 4。

`DBL-2` はコードとテストは済んでいるが、**ゲートの耳での確認が `DBL-4` 待ち**
（ホストで鳴らせるようになるまで判断できない）。`✅ ❓` はその状態。

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
| UI-1 | テーマトークンと gallery の中身（クレートと空の gallery は INFRA-1 で済み） | 🟡 |
| UI-2 | Lucide アイコン埋め込みと定数生成 | ⬜ UI-1 |
| UI-3 | 共通の入力ふるまい（ドラッグ / 微調整 / リセット / ツールチップ / 値入力 / ジェスチャー通知） | ⬜ UI-1 |
| UI-4 | `Knob`（大小 2 サイズ） | ⬜ UI-3 |
| UI-5 | `Bar` | ⬜ UI-3 |
| UI-6 | `SegmentedControl` | ⬜ UI-1 |
| UI-7 | `PolarField`（領域知識を持たない極座標フィールド） | ⬜ UI-3 |
| UI-8 | `Meter` — **Doubler は使わない** | ⬜ UI-1 |
| UI-9 | `ToggleSwitch` — **Doubler は使わない** | ⬜ UI-1 |

### Doubler — `../../plugins/doubler/docs/implementation/doubler-plan.md`

| ID | 単位 | 状態 |
|---|---|---|
| DBL-1 | リングバッファ遅延線と Hermite 補間 | ✅ `a84779d` |
| DBL-2 | 回転タップピッチシフタ（**ゲート通過**） | ✅ `f2501d6` |
| DBL-3 | ボイスエンジン（形状表・実効値・パン則・ゲイン補償） | ✅ `3aec7af` |
| DBL-4 | nih-plug ラッパと配線（**ここで音が出る**。UI 無し） | ✅ `556c5a0` |
| DBL-5 | Humanize | ✅ |
| DBL-6 | Source モード（Mono Sum / True Stereo） | 🟡 |
| DBL-7 | Tone と Tone Spread | 🟡 |
| DBL-8 | スムージングとサンプルレート／ブロックサイズ非依存の詰め | ⬜ DBL-5, DBL-6, DBL-7 |
| DBL-9 | UI マクロ層（ノブとセグメント） | ⬜ DBL-4, UI-4, UI-6, UI-2 |
| DBL-10 | UI Voice Field | ⬜ DBL-9, DBL-5, DBL-6, UI-7 |
| DBL-11 | UI Detail 層（ボイス表） | ⬜ DBL-10, UI-5 |
| DBL-12 | CPU 予算の確認（criterion） | ⬜ DBL-8 |
| DBL-13 | 既定値の詰めと実機確認 | ⬜ DBL-12, DBL-11 |
