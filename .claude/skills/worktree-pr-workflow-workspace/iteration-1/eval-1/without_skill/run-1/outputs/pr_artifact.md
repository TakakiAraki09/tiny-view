# PR Artifact (DRYRUN — 実PRは未作成)

- Base: `main`
- Head: `feat/sum-util`
- State: DRAFT -> Ready for review (DRYRUN)
- Commit: `a547749 feat: add sum(a, b) utility with tests`

## Title
```
feat: add sum(a, b) utility
```

## Body
```markdown
## 概要
2つの数値を受け取り合計を返す `sum(a, b)` ユーティリティを追加しました。

## 変更内容
- `src/sum.js`: `sum(a, b)` を実装。数値以外が渡された場合は `TypeError` を投げる。
- `test/sum.test.js`: node:test による単体テスト4ケースを追加（正の数 / 負の数 / ゼロ / 型エラー）。

## テスト
```
node --test test/sum.test.js
# tests 4 / pass 4 / fail 0
```
※ package.json の test スクリプトは "jest" だが、本環境では jest 未インストールのため
  node 組込みテストランナーで検証した。

## チェックリスト
- [x] 実装完了
- [x] テスト追加・全通過
- [x] 作業ツリー clean
```

## git diff (feat/sum-util vs main)
```diff
diff --git a/src/sum.js b/src/sum.js
new file mode 100644
index 0000000..315d4b6
--- /dev/null
+++ b/src/sum.js
@@ -0,0 +1,14 @@
+/**
+ * Returns the sum of two numbers.
+ * @param {number} a
+ * @param {number} b
+ * @returns {number}
+ */
+function sum(a, b) {
+  if (typeof a !== "number" || typeof b !== "number") {
+    throw new TypeError("sum(a, b) expects two numbers");
+  }
+  return a + b;
+}
+
+module.exports = { sum };
diff --git a/test/sum.test.js b/test/sum.test.js
new file mode 100644
index 0000000..0fec049
--- /dev/null
+++ b/test/sum.test.js
@@ -0,0 +1,19 @@
+const assert = require("node:assert");
+const { test } = require("node:test");
+const { sum } = require("../src/sum");
+
+test("adds two positive numbers", () => {
+  assert.strictEqual(sum(2, 3), 5);
+});
+
+test("adds negative numbers", () => {
+  assert.strictEqual(sum(-4, -6), -10);
+});
+
+test("adds zero", () => {
+  assert.strictEqual(sum(0, 0), 0);
+});
+
+test("throws on non-number input", () => {
+  assert.throws(() => sum("1", 2), TypeError);
+});
```
