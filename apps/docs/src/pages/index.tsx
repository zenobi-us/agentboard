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
          title={<>ClankPipe collects work items and runs workspace actions.</>}
          subtitle="A Bun CLI for local agent queues: Jira, QMD collections, and GitHub Issues in; a local Store, Git worktrees, and shell commands out."
        >
          <HeroActions>
            <HeroAction primary asChild>
              <Link href="/quickstart">
                Quickstart
              </Link>
            </HeroAction>
            <HeroAction href="https://github.com/zenobi-us/clankpipe">
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

