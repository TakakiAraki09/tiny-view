#!/usr/bin/env bash
# PreToolUse hook: 編集系ツール (Edit/Write/NotebookEdit) を
# primary worktree (main checkout) で実行しようとしたらブロックする。
# リンク済み git worktree 内でのみ編集を許可することで「worktree 化必須」を強制する。
#
# 配置: .claude/hooks/require-worktree.sh (実行権限を付与すること)
# 登録: .claude/settings.json の PreToolUse hook (assets/settings.json 参照)
# <REPO_NAME> をリポジトリ名に置換して使う。

set -euo pipefail

input=$(cat)

# tool_input から対象パスを取得 (file_path / notebook_path)。無ければ cwd。
target=$(printf '%s' "$input" | python3 -c '
import sys, json
try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(0)
ti = d.get("tool_input", {}) or {}
print(ti.get("file_path") or ti.get("notebook_path") or d.get("cwd") or "")
')

[ -z "$target" ] && exit 0

if [ -d "$target" ]; then
  dir="$target"
else
  dir=$(dirname "$target")
fi

# 対象が git 管理下でなければ何もしない (リポジトリ外の編集はブロックしない)。
gd=$(git -C "$dir" rev-parse --absolute-git-dir 2>/dev/null) || exit 0
gc=$(git -C "$dir" rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || exit 0

# git-dir と git-common-dir が一致 = primary worktree。リンク worktree では異なる。
if [ "$gd" = "$gc" ]; then
  cat >&2 <<'MSG'
✋ worktree 必須: いま primary worktree (main checkout) を編集しようとしています。
編集の前に専用の git worktree を作成し、そこへ移動してから作業してください。例:

  git worktree add ../<REPO_NAME>-<task> -b feat/<task>
  cd ../<REPO_NAME>-<task>

リンク済み worktree 内であれば編集が許可されます。
MSG
  exit 2
fi

exit 0
