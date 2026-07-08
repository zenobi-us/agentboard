import { CtaHero } from "../components/CtaHero";
import { HeroAction } from "../components/HeroAction";
import { HeroActions } from "../components/HeroActions";
import { Logo } from "../components/Logo";
import { Section } from "../components/Section";
import { Site } from "../components/Site";
import { WorkspaceConfigExample } from "../components/WorkspaceConfigExample";
import { Link } from "fumapress/client";

export default function HomePage() {
  return (
    <Site className="flex flex-col grow items-center justify-center">
      <Section className="flex grow pt-6 lg:pt-10 items-center">
        <CtaHero
          tagline={<Logo suffix={<span className="text-rp-foam">.</span>} />}
          title={<>AgentBoard collects work items and runs workspace actions.</>}
          subtitle="A Rust CLI for local agent queues: Jira, Linear, markdown, GitHub Projects, and GitHub Issues in; synced store, worktrees, and commands out."
        >
          <HeroActions>
            <HeroAction primary asChild>
              <Link href="/quickstart">
                Quickstart
              </Link>
            </HeroAction>
            <HeroAction href="https://github.com/zenobi-us/agentboard">
              GitHub
            </HeroAction>
          </HeroActions>
        </CtaHero>
        <div>
          <WorkspaceConfigExample />
        </div>
      </Section>
    </Site>
  );
}

