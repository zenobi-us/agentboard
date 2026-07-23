#!/bin/bash

# # Create Demo Github Repo
#
# 1. create new private repo
# 2. clone that to local machine
# 3. copy this dir to the new repo
# 4. commit and push the changes to the new repo
#
## Initialise repo
#
# 1. for task in ./.issues/*.json; do gh issue create --title "$(jq -r '.title' "$task")" --body "$(jq -r '.body' "$task")"; done
#
## Start Agentboard
#
# 1. agentboard watch .agentboard.toml --interval 15s
#
## Label Some of the issues
#
# 1. gh issue edit 1 --add-label "agentboard:ready-for-agent" gg# 2. watch new terminal spawn with pi starting to work on the issue
# 2. watch new terminal spawn with pi starting to work on the issue
