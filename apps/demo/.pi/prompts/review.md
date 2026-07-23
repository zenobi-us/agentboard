
Review GitHub issue and the associated PR.

Act as an independent reviewer. 
Do not edit files or commit changes. 
Read the issue and PR with gh, inspect the diff, and run the exact task test plus ./test.sh.

If changes are required:
- comment precise findings on the issue
- remove agentboard:ready-for-review, and
- add agentboard:changes-requested plus agentboard:ready-for-agent. 

If the change passes:
- comment the review and test evidence, 
- remove agentboard:ready-for-review and agentboard:changes-requested when present, and
- add agentboard:review-complete. 

Do not merge the PR. 

Stop after reporting the transition you applied.

Issue: $1
PR: $2
UserRequest: $ARGUMENTS
