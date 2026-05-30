---
name: issue-batch-pr
description: 特定ラベル（既定 prepare-development）の付いた GitHub issue を一覧取得し、1 issue ＝ 1 subagent で worktree-pr-workflow を並列に回して、それぞれ実装〜PR 作成まで一括処理するオーケストレーションスキル。「ラベルの付いた issue を全部 PR にして」「prepare-development の issue をまとめて実装して」「issue 一覧を一気に PR 化して」「溜まってる issue を並列でさばいて」「ラベル◯◯の issue を片っ端から実装して提出まで」など、複数 issue を一括で実装→PR 化したい依頼で使う。明示的に「並列」「一括」と言われなくても、複数 issue をまとめて実装提出してほしいニュアンスがあれば積極的にトリガーすること。単一タスクを実装して PR にするだけなら worktree-pr-workflow を直接使う。
---

# Issue Batch → PR Orchestration

特定ラベルの付いた GitHub issue 群を受け取り、**各 issue を独立した subagent に割り当てて並列に実装〜PR 作成まで回す**オーケストレーションスキル。

このスキル自身はコードを書かない。役割は **PM / ディスパッチャ**で、(1) 対象 issue の収集と確認、(2) issue 単位の subagent への委譲、(3) 結果の集約と報告 に責任を持つ。実装〜検品〜PR 化の一気通貫は、各 subagent が `worktree-pr-workflow` スキルに従って行う。

## このスキルが扱うもの・扱わないもの

- **扱う**: 「ラベルで絞った複数 issue を、まとめて実装して PR 化する」依頼。issue の収集・確認ゲート・並列ディスパッチ・結果集約。
- **扱わない**:
  - 単一タスク / 単一 issue を実装して PR にする → `worktree-pr-workflow` を直接
  - 既存コミットを PR にするだけ → `/pr`
  - issue を眺める・トリアージするだけ（実装しない） → 通常の対話
- 依頼の粒度が曖昧なら短く確認する。例:「`prepare-development` ラベルの issue を全部、実装から PR 化まで一括で回す依頼でよいですか？」

## 絶対原則（破ってはならないガードレール）

毎フェーズで意識する。多数の PR を一気に立てる強い副作用を持つスキルなので、安全側に倒す。

- **着手前に必ず確認ゲートを通す**: 対象 issue の一覧（番号・タイトル・件数）を提示し、ユーザーから一度だけ Go を取ってから初めてディスパッチする。理由 — ラベルに 10 件付いていれば 10 本の PR が一気に立つ。意図しない大量 PR を防ぐため、量と中身を人が見てからにする。
- **全 issue で必ず PR を残す**: 実装が完走しても行き詰まっても、各 issue は**最低でもドラフト PR を残す**。完走したものだけ Ready 化し、未完のものはドラフトのまま「要確認」として残す。理由 — 部分的にでも進んだ成果と状況を人が引き取れる形にするため。何も残さず失敗で終わらせない。
- **1 件の失敗で全体を止めない**: ある issue の subagent がコケても、他の subagent は止めない。失敗は記録して最後に集約報告する（部分成功を許容）。理由 — 並列でさばく以上、一部がコケるのは前提。1 件のために全体を巻き戻すのは損失が大きい。
- **issue ごとに隔離する**: 各 subagent は `worktree-pr-workflow` に従い、**それぞれ別の worktree とブランチ**で作業する。ブランチ名・worktree パスには issue 番号を含め、衝突を防ぐ。理由 — 並列実行でファイル・ブランチが競合すると全体が壊れるため。
- **main は集約して締める**: subagent の生出力をそのまま最終成果物にしない。全 subagent の結果を受け取り、表に集約してから完了報告する。理由 — ユーザーが「何件成功し、どれが要確認か」を一目で把握できる形にするのがこのスキルの価値だから。

## ワークフロー全体像

```
Phase 0  前提確認 ── gh認証 / リポジトリ確認 / ラベル決定
Phase 1  Issue 収集 ── ラベルで絞って一覧取得 → ユーザーへ提示
Phase 2  確認ゲート ── 件数・中身を見せて Go を取る（必須）
Phase 3  並列ディスパッチ ── 1 issue = 1 subagent を「同一ターンで」全部起動
Phase 4  結果集約 ── 各 subagent の構造化結果を受け取り表にまとめる
Phase 5  完了報告 ── 成功(Ready)/要確認(ドラフト)/失敗を一覧化 + 後始末方針
```

---

## Phase 0: 前提確認

1. **gh 認証を確認**（`gh auth status`）。未認証ならユーザーに `! gh auth login` を促して止める。
2. **対象リポジトリを確認**。既定はカレント git リポジトリ。別リポジトリ指定があれば `--repo` 相当で扱う。
3. **対象ラベルを決める**。既定は `prepare-development`。依頼に別ラベルの指定があればそれを使う。
4. **ドライラン指示の有無を確認**。依頼やテストに「ドライラン」「dry-run」「実 PR は作らない」があれば、各 subagent に `worktree-pr-workflow` のドライランモードを使うよう指示する（後述）。

## Phase 1: Issue 収集

対象ラベルの付いた **open** issue を一覧取得する。

```bash
gh issue list --label "prepare-development" --state open \
  --json number,title,labels,url --limit 100
```

- 取得結果を `番号 / タイトル / URL` の表に整形する。
- **0 件なら**ディスパッチに進まず、「対象 issue なし」と報告して終了する。
- PR(Pull Request) を除外したい場合は issue のみ取得する（`gh issue list` は PR を含めない）。

## Phase 2: 確認ゲート（必須）

収集した一覧をユーザーに提示し、着手の許可を取る。**ここを飛ばして Phase 3 に進んではいけない。**

提示するもの:

- 対象件数と issue 一覧（番号・タイトル）
- これから「各 issue を別 worktree で実装し、それぞれドラフト PR を作る（検品通過分は Ready 化）」という実行内容
- 完全並列で走る旨（件数が多いと重い旨も添える）

そのうえで「この N 件を実装して PR 化します。進めて OK ですか？」と一度だけ確認する。

- 件数が多すぎる（例: 20 件超）ときは、バッチ分割や対象の絞り込みを提案してよい。
- ユーザーが対象の取捨選択をしたら、その指示に従って対象集合を確定する。

## Phase 3: 並列ディスパッチ

確定した issue 集合について、**1 issue = 1 subagent** を **同一ターンで全部** 起動する（完全並列）。後から小出しにせず、まとめて投げることでウォールクロックを最小化する。

各 subagent へ渡すプロンプトは、以下の「subagent 委譲テンプレート」に従う。要点:

- subagent に `worktree-pr-workflow` スキル（`.claude/skills/worktree-pr-workflow/SKILL.md`）を読み、**その 1 issue について Phase 0〜8 を完走させる**。
- ブランチ名・worktree パスに **issue 番号**を必ず含める（衝突回避）。例: `feat/issue-<N>-<slug>`、worktree は `../<repo>-issue-<N>`。
- PR 本文に `Closes #<N>` を入れ、issue と PR を紐づける。
- **行き詰まっても必ずドラフト PR を残して終える**（`worktree-pr-workflow` の中断時挙動と一致）。
- **検品（レビュー）の出力で終わらせない**。`worktree-pr-workflow` の Phase 6（検品）まで来たら、必ず Phase 7（判定）→ Phase 8（DRYRUN 時は PR 相当の成果物提示）まで進み、**最後に下記 JSON を返す**ことを subagent の終了条件にする。理由 — レビューエージェントの生出力で会話を打ち切る失敗が起きやすく、それだと main の集約が成立しないため。「JSON を返さずに終わったら未完了」と明示する。
- 結果を**構造化して返す**よう指示する（下記スキーマ）。subagent の最終メッセージはユーザーには見えず main への戻り値になるので、生の散文ではなく機械可読なまとめを返させる。

### subagent 委譲テンプレート

```
あなたは GitHub issue #<N> を 1 件、実装から PR 作成まで完走させる担当です。

- 対象 issue: #<N> 「<title>」
  <issue body の要約 or URL>
- 対象リポジトリ: <repo path / slug>
- 従うべき手順: .claude/skills/worktree-pr-workflow/SKILL.md を読み、その Phase 0〜8 を
  この 1 issue について実行してください。
- ブランチ名は feat/issue-<N>-<slug>、worktree は ../<repo>-issue-<N> を使い、
  必ず issue 番号を含めて他の作業と衝突しないようにすること。
- PR 本文の末尾に `Closes #<N>` を必ず入れること。
- 検品 4 軸を通過したら gh pr ready で Ready 化、行き詰まったらドラフトのまま残すこと。
  いずれの場合も「PR を 1 本残した状態」で終えること（PR を作らずに失敗終了しない）。
- <ドライラン時>: worktree-pr-workflow のドライランモードで実行し、push / pr create /
  pr ready は DRYRUN: ログに置換すること。

検品（レビュー）で終わらず、判定とドラフトPR相当の提示まで必ず進めてください。
そのうえで、**最終メッセージは次の JSON だけ**にしてください（レビュー文で終わらない）:
{
  "issue": <N>,
  "title": "<title>",
  "status": "ready" | "draft" | "failed",
  "pr_url": "<URL or null>",
  "summary": "<何をしたか 1-2 行>",
  "remaining": "<未解決点があれば。なければ null>"
}
```

- `agentType` は汎用エージェント（general-purpose / claude）でよい。Skill / Bash / Edit など全ツールが使える必要がある。
- 並列起動時は同時実行数の上限にかかることがあるが、超過分は自動的に順番待ちになるので、件数分まとめて起動してよい。

## Phase 4: 結果集約

全 subagent の戻り値（JSON）を受け取り、`issue / status / pr_url / summary / remaining` で 1 表にまとめ直す。

- **JSON で返ってこなかった subagent は実体から status を再構成する**（レビュー出力で打ち切る subagent が一定割合いるため、ここは必ず行う）:
  - `git worktree list` で issue 番号付き worktree が作られたか
  - 当該ブランチに実装コミットが積まれているか（`git -C <worktree> log --oneline`）
  - worktree 内で検証コマンド（例: `node test.js` / `cargo test`）が通るか
  - 実装コミット有り＋検証通過なら `ready` 相当、コミットは有るが検証 NG / 未完なら `draft`、worktree もコミットも無ければ `failed` と判定する。
- `status` で分類: **ready（完走・Ready化）/ draft（要確認・ドラフト残し）/ failed（PR すら作れず）**。
- failed が出た場合、その理由（subagent の summary / remaining）を必ず拾う。

## Phase 5: 完了報告（必須の締め）

最後に必ず以下を構造化して提示する。subagent の生出力で会話を終えない。

- **サマリ**: 対象 N 件中 — Ready X 件 / 要確認(ドラフト) Y 件 / 失敗 Z 件
- **一覧表**: issue 番号 → タイトル → status → PR URL → 補足（要確認/失敗の理由）
- **要対応**: ドラフトのまま残った PR と、その残課題
- **後始末方針**: 各 worktree は `git worktree remove <path>`（マージ後でよい旨を添える。ユーザーが続きを触る可能性があるので**勝手に削除しない**）

---

## ドライランモード（評価・テスト用）

このスキルは多数の実 PR を作る副作用を持つため、評価・テスト時は**ドライラン**で回す。依頼やテストに「ドライラン」「dry-run」「実 PR は作らない」等があれば:

- **issue 収集（Phase 1）と確認ゲート（Phase 2）は通常どおり**実行する（read-only なので安全）。テスト用にローカルの使い捨て git リポジトリと擬似 issue リストが与えられた場合は、`gh issue list` の代わりにそれを対象集合として扱う。
- 各 subagent には `worktree-pr-workflow` の**ドライランモード**を使うよう指示する。worktree 作成・編集・ローカル検証・commit は実行してよいが、`git push` / `gh pr create` / `gh pr ready` は `DRYRUN:` ログに置換させる。
- **集約・報告（Phase 4-5）は通常どおり**。各 issue について「PR 相当の成果物（タイトル・本文・差分）」がログに出ていることを確認し、status を `ready / draft` 相当に分類して報告する。

目的は「収集 → 確認ゲート → 並列ディスパッチ → 集約報告」というオーケストレーションのロジックが正しく機能するかを、GitHub を汚さずに検証することにある。

## チェックリスト（各実行で自問する）

- [ ] gh 認証と対象リポジトリ・ラベルを確定したか
- [ ] ラベルで絞った issue 一覧をユーザーに提示し、Go を取ってから着手したか（確認ゲート）
- [ ] 0 件のときにディスパッチせず終了したか
- [ ] 1 issue = 1 subagent を同一ターンでまとめて起動したか（完全並列）
- [ ] 各 subagent に worktree-pr-workflow を渡し、ブランチ/worktree に issue 番号を含めさせたか
- [ ] 全 issue が「最低でもドラフト PR を残す」状態で終わったか
- [ ] 1 件の失敗で全体を止めず、結果を集約して報告したか
- [ ] ドライラン指示があれば各 subagent に伝播したか
