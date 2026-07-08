import { ServerCodeBlock } from "fumadocs-ui/components/codeblock.rsc";

const jiraWorkspaceConfig = `[[sources]]
id = "jira-ready"

[sources.source]
kind = "jira"
site = "https://example.atlassian.net"
jql = "project = AB AND statusCategory = Todo"

[[sources.actions]]
uses = "agentboard/create-worktree"

[sources.actions.with]
repo = "~/dev/myrepo"
root = "~/dev/myrepo.trees/{{ item.id }}"
branch = "feat/{{ item.id }}-{{ item.summary | slugify }}"

[[sources.actions]]
uses = "agentboard/run-cmd"

[sources.actions.with]
cmd = '''
zellij action new-tab 
  --name "{{ item.id }}"
  --cwd "~/dev/myrepo.trees/{{ item.id }}" 
  pi
'''
`;

export async function WorkspaceConfigExample() {
  return <ServerCodeBlock code={jiraWorkspaceConfig} lang="toml" />;
}
