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

Run these steps if the repository rename must be reversed.

1. Pause ClankPipe automation.

2. Restore the repository name:

   ```sh
   gh repo rename agentboard --repo zenobi-us/clankpipe --confirm
   ```

3. Restore `.agentboard.toml` to the pre-migration configuration. Set these values:

   ```toml
   query = '''repo:zenobi-us/agentboard is:open (label:"ready-for-agent" OR label:"agentboard:changes-requested") -label:"agentboard:implementing" -label:"agentboard:ready-for-review" -label:"agentboard:reviewing" sort:created-asc'''
   branch = "agentboard/{{ item.id | slugify }}"
   ```

   Set the review query to `repo:zenobi-us/agentboard` with `agentboard:ready-for-review` and `agentboard:reviewing`. Restore the old status map keys.

4. Restore each local remote:

   ```sh
   git remote set-url origin git@github.com:zenobi-us/agentboard.git
   ```

5. Restore labels on open issues. Add the old label before you remove the new label:

   ```sh
   gh issue edit ISSUE --repo zenobi-us/agentboard \
     --add-label ready-for-agent \
     --remove-label clankpipe:ready-for-agent
   gh issue edit ISSUE --repo zenobi-us/agentboard \
     --add-label agentboard:changes-requested \
     --remove-label clankpipe:changes-requested
   ```

   Repeat the second command for `implementing`, `ready-for-review`, `reviewing`, `review-complete`, and `cleanup-approved`.

6. Restart automation with the restored `.agentboard.toml`.

Do not delete the new labels until open issue state has been checked after rollback.
