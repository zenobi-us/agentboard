import { defineConfig } from "fumapress";
import { llmsPlugin } from "fumapress/plugins/llms.txt";
import { flexsearchPlugin } from "fumapress/plugins/flexsearch";
import { fumadocsMdx } from "fumapress/adapters/mdx";
import { update } from "fumadocs-core/source";
// don't worry if this file is missing, we will run the dev command later to generate this file
import { actionDocs, docs, sourceDocs } from "./.source/server";

const docsSource = update(docs.toFumadocsSource())
  .page((page) => page)
  .build();

const packageDocName = (path: string, prefix: string) =>
  (path.split("/")[0] ?? path).replace(prefix, "").replaceAll("_", "-");

const sourceDocsSource = update(sourceDocs.toFumadocsSource())
  .files((files) => files.filter((file) => file.path.endsWith("docs.md")))
  .page((page) => {
    const name = packageDocName(page.path, "agentboard-source-");
    const slugs = ["sources", name];

    return {
      ...page,
      slugs,
      path: slugs.join("/"),
      data: {
        ...page.data,
        info: {
          ...page.data.info,
          path: `/${slugs.join("/")}`,
        },
      },
    };
  })
  .build();

const actionDocsSource = update(actionDocs.toFumadocsSource())
  .files((files) => files.filter((file) => file.path.endsWith("docs.md")))
  .page((page) => {
    const name = packageDocName(page.path, "agentboard-action-");
    const slugs = ["actions", name];

    return {
      ...page,
      slugs,
      path: slugs.join("/"),
      data: {
        ...page.data,
        info: {
          ...page.data.info,
          path: `/${slugs.join("/")}`,
        },
      },
    };
  })
  .build();

export default defineConfig({
  content: {
    docs: docsSource,
    sourceDocs: sourceDocsSource,
    actionDocs: actionDocsSource,
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
