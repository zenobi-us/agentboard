# AgentBoard to ClankPipe repository migration

Run this migration during a short automation pause.

## Apply

1. Confirm the current repository and backup its issue labels:

   ```sh
   gh repo view zenobi-us/agentboard
   gh label list --repo zenobi-us/agentboard --limit 100 > /tmp/agentboard-labels.txt
   ```

2. Rename the repository. GitHub keeps redirects for old repository URLs:

   ```sh
   gh repo rename clankpipe --repo zenobi-us/agentboard --confirm
   ```

3. From each local clone, run the migration script. It creates new labels before it removes old labels, so open issue workflow state stays represented during the change:

   ```sh
   .github/tasks/migrate-to-clankpipe.sh --apply
   ```

4. Restart ClankPipe with the updated `.agentboard.toml`. New automation queries use `zenobi-us/clankpipe` and `clankpipe:*` labels.

5. Check open issues and branches:

   ```sh
   gh issue list --repo zenobi-us/clankpipe --state open --limit 100 --json number,labels
   git remote -v
   git ls-remote --heads origin 'clankpipe/*'
   ```

Existing `agentboard/*` branches remain valid. New workflow-created branches use `clankpipe/*`.

## Rollback

Before deleting any old labels, restore the repository name and labels:

```sh
gh repo rename agentboard --repo zenobi-us/clankpipe --confirm
```

For each issue, add its old label before removing the matching `clankpipe:*` label:

```sh
gh issue edit ISSUE --repo zenobi-us/agentboard \
  --add-label agentboard:ready-for-agent \
  --remove-label clankpipe:ready-for-agent
```

Repeat for `implementing`, `changes-requested`, `ready-for-review`, `reviewing`, `review-complete`, and `cleanup-approved`. Then restore each local remote:

```sh
git remote set-url origin git@github.com:zenobi-us/agentboard.git
```

Do not delete the new labels until issue state has been checked after rollback.
