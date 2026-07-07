import { AboveTheFold } from "../components/AboveTheFold";
import { CtaHero } from "../components/CtaHero";
import { HeroAction } from "../components/HeroAction";
import { HeroActions } from "../components/HeroActions";
import { Logo } from "../components/Logo";
import { Section } from "../components/Section";
import { Site } from "../components/Site";

export default function HomePage() {
  return (
    <Site>
      <Section className="pt-6 lg:pt-10">
        <CtaHero
          tagline={<Logo suffix={<span className="text-rp-foam">.</span>} />}
          title={<>AgentBoard collects work items and runs workspace actions.</>}
          subtitle="A Rust CLI for local agent queues: Jira, Linear, markdown, GitHub Projects, and GitHub Issues in; synced store, worktrees, and commands out."
        >
          <HeroActions>
            <HeroAction primary href="/docs/quickstart">
              Quickstart
            </HeroAction>
            <HeroAction href="https://github.com/zenobi-us/agentboard">
              GitHub
            </HeroAction>
          </HeroActions>
        </CtaHero>
        <div className="rounded-2xl border border-rp-muted/30 bg-rp-surface p-5 font-mono text-sm leading-7 text-rp-text shadow-2xl shadow-black/20">
          <p className="text-rp-foam">~/.config/agentboard/work.toml</p>
          <p>[[sources]]</p>
          <p>id = "ready"</p>
          <p>query = "status:ready"</p>
          <p className="mt-4">[[sources.actions]]</p>
          <p>uses = "agentboard/create-worktree"</p>
          <p className="mt-4 text-rp-subtle">branch = "&#123;&#123; item.id &#125;&#125;/&#123;&#123; item.title | slugify &#125;&#125;"</p>
        </div>
      </Section>
      <AboveTheFold />
    </Site>
  );
}
