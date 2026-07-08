import { defineConfig } from "fumapress";
import { llmsPlugin } from "fumapress/plugins/llms.txt";
import { flexsearchPlugin } from "fumapress/plugins/flexsearch";
import { fumadocsMdx } from "fumapress/adapters/mdx";
import { update } from "fumadocs-core/source";
// don't worry if this file is missing, we will run the dev command later to generate this file
import { docs } from "./.source/server";

const docsSource = update(docs.toFumadocsSource())
  .page((page) => page)
  .build();

export default defineConfig({
  content: {
    docs: docsSource,
  },
  mode: "static",
  site: {
    baseUrl: "https://zenobi-us.github.io/agentboard/",
    name: "AgentBoard",
    git: {
      repo: "agentboard",
      user: "zenobi-us",
      branch: "main",
    },
  },
  meta: {
    root() {
      return (
        <>
          <link rel="preconnect" href="https://fonts.googleapis.com" />
          <link
            rel="preconnect"
            href="https://fonts.gstatic.com"
            crossOrigin=""
          />
          <link
            href="https://fonts.googleapis.com/css2?family=Geist+Mono:wght@100..900&family=Geist:wght@100..900&display=swap"
            rel="stylesheet"
          />
        </>
      );
    },
    page() {
      return <></>;
    },
  },
})
  .plugins(flexsearchPlugin(), llmsPlugin())
  .adapters(fumadocsMdx());
