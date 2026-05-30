# PR Artifact (DRYRUN — 実PRは未作成)

> このPRは作成していません。DRYRUN として、push/PR作成に相当する内容（タイトル・本文・差分）を提示します。
> 実行されたであろうコマンド:
> - `DRYRUN: git push -u origin feat/implement-apply-discount`
> - `DRYRUN: gh pr create --draft --base main --title "..." --body "..."`
> - `DRYRUN: gh pr ready <pr-number>`  （検品4軸 PASS のため）

## PR Title (English)

```
feat: implement applyDiscount with percent range validation
```

## PR Body

```markdown
## 概要

`src/discount.js` に `applyDiscount(price, percent)` を実装し、`test.js` の全テストを通るようにした。

## 詳細

- `applyDiscount(price, percent)` を追加。
  - 割引後価格 = `price - (price * percent) / 100`
  - `percent` は `0..100` の閉区間。範囲外（`< 0` または `> 100`）は `RangeError` を投げる。
- 検証: `npm test`（`node test.js`）で 6/6 PASS。
  - `applyDiscount(1000, 20) === 800`
  - `applyDiscount(0, 50) === 0`
  - `percent < 0` / `percent > 100` で例外
  - 境界 `percent = 0`（割引なし）/ `percent = 100`（全割引）も有効

## 補足

- 初回実装で境界 `percent = 0` を誤って除外していたため（`<= 0` → `< 0`）、修正ループ 1 回で収束した。
- 非数値・NaN の `percent` は現状バリデーションしていない（本タスクの受け入れ条件外）。必要なら別途追加可能。
```

- Base branch: `main`
- Head branch: `feat/implement-apply-discount`
- Draft: yes（検品 PASS 後に Ready 化 = DRYRUN）

## git diff (main...HEAD)

```diff
diff --git a/src/discount.js b/src/discount.js
new file mode 100644
index 0000000..850f514
--- /dev/null
+++ b/src/discount.js
@@ -0,0 +1,7 @@
+export function applyDiscount(price, percent) {
+  // percent must be within the inclusive range 0..100
+  if (percent < 0 || percent > 100) {
+    throw new RangeError(`percent must be between 0 and 100, got ${percent}`);
+  }
+  return price - (price * percent) / 100;
+}
```
