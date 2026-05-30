# PR Artifact (DRYRUN — not actually created)

- Base: `main`
- Head: `feat/apply-discount`
- Draft -> Ready (tests green)
- Status: DRYRUN（実 push / 実 PR 作成は行っていない）

## Title

```
feat: implement applyDiscount in src/discount.js
```

## Body

```markdown
## 概要
`test.js` が要求する `applyDiscount(price, percent)` を `src/discount.js` に実装した。

## 変更点
- `src/discount.js` を新規追加
  - `applyDiscount(price, percent)` = `price * (1 - percent / 100)`
  - `percent` が 0..100 の範囲外なら `RangeError` を throw
  - `price` / `percent` の型ガード（非数値は `TypeError`）

## テスト
`npm test`（= `node test.js`）

- [x] applyDiscount(1000, 20) === 800
- [x] applyDiscount(0, 50) === 0
- [x] percent < 0 / > 100 で例外 throw
- [x] 境界値 0% / 100% は throw しない

結果: 全アサーション PASS（EXIT=0, `All tests passed`）

## 検証ログ
- 実装前: `npm test` → FAIL (ERR_MODULE_NOT_FOUND)
- 実装後: `npm test` → PASS
- 修正ループ反復: 0 回（初回実装でグリーン）
```

## git diff (main..feat/apply-discount)

```diff
diff --git a/src/discount.js b/src/discount.js
new file mode 100644
index 0000000..eb572e6
--- /dev/null
+++ b/src/discount.js
@@ -0,0 +1,20 @@
+/**
+ * Apply a percentage discount to a price.
+ *
+ * @param {number} price   Base price (non-negative number).
+ * @param {number} percent Discount percentage, must be within 0..100.
+ * @returns {number} Discounted price.
+ * @throws {RangeError} If percent is outside the 0..100 range.
+ */
+export function applyDiscount(price, percent) {
+  if (typeof price !== "number" || Number.isNaN(price)) {
+    throw new TypeError("price must be a number");
+  }
+  if (typeof percent !== "number" || Number.isNaN(percent)) {
+    throw new TypeError("percent must be a number");
+  }
+  if (percent < 0 || percent > 100) {
+    throw new RangeError("percent must be between 0 and 100");
+  }
+  return price * (1 - percent / 100);
+}
```

## DRYRUN commands (push / PR)

```
DRYRUN: git push -u origin feat/apply-discount
DRYRUN: gh pr create --draft --base main --head feat/apply-discount \
          --title 'feat: implement applyDiscount in src/discount.js' --body-file <pr_body>
DRYRUN: gh pr ready feat/apply-discount   # tests green -> mark Ready for review
```
