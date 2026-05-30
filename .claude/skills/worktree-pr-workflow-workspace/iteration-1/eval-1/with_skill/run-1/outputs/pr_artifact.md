# PR Artifact (reconstructed from worktree) — happy-path / with_skill

- repo: /tmp/wpr-happy-withskill  / worktree: /tmp/wpr-happy-withskill-add-sum  / branch: feat/add-sum  / base: main
- DRYRUN: git push -u origin feat/add-sum
- DRYRUN: gh pr create --draft --base main
- DRYRUN: gh pr ready (検品4軸PASS後)

## Commits
52d3ee4 ✨ feat: add sum(a, b) utility with tests
02a3b29 chore: initial commit with package.json and src

## Diff vs main
```diff
diff --git a/src/sum.js b/src/sum.js
new file mode 100644
index 0000000..a4c234f
--- /dev/null
+++ b/src/sum.js
@@ -0,0 +1,9 @@
+// sum(a, b): receives two numbers and returns their sum.
+function sum(a, b) {
+  if (typeof a !== "number" || typeof b !== "number") {
+    throw new TypeError("sum(a, b) expects both arguments to be numbers");
+  }
+  return a + b;
+}
+
+module.exports = { sum };
diff --git a/src/sum.test.js b/src/sum.test.js
new file mode 100644
index 0000000..2a3902f
--- /dev/null
+++ b/src/sum.test.js
@@ -0,0 +1,23 @@
+const { sum } = require("./sum");
+
+describe("sum", () => {
+  test("adds two positive numbers", () => {
+    expect(sum(2, 3)).toBe(5);
+  });
+
+  test("adds negative numbers", () => {
+    expect(sum(-4, -6)).toBe(-10);
+  });
+
+  test("handles zero", () => {
+    expect(sum(0, 0)).toBe(0);
+  });
+
+  test("handles floating point", () => {
+    expect(sum(0.1, 0.2)).toBeCloseTo(0.3);
+  });
+
+  test("throws on non-number input", () => {
+    expect(() => sum("1", 2)).toThrow(TypeError);
+  });
+});
diff --git a/test/sum.assert.test.js b/test/sum.assert.test.js
new file mode 100644
index 0000000..e84381d
--- /dev/null
+++ b/test/sum.assert.test.js
@@ -0,0 +1,12 @@
+// Runnable verification using node's built-in assert.
+// Used because jest is configured but not installed in this throwaway repo.
+const assert = require("assert");
+const { sum } = require("../src/sum");
+
+assert.strictEqual(sum(2, 3), 5, "2 + 3 should be 5");
+assert.strictEqual(sum(-4, -6), -10, "-4 + -6 should be -10");
+assert.strictEqual(sum(0, 0), 0, "0 + 0 should be 0");
+assert.ok(Math.abs(sum(0.1, 0.2) - 0.3) < 1e-9, "0.1 + 0.2 should be ~0.3");
+assert.throws(() => sum("1", 2), TypeError, "non-number input should throw");
+
+console.log("All sum() assertions passed.");
```
