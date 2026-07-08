import { defineConfig } from "fumapress";
import { llmsPlugin } from "fumapress/plugins/llms.txt";
import { flexsearchPlugin } from "fumapress/plugins/flexsearch";
import { fumadocsMdx } from "fumapress/adapters/mdx";
import {
  update,
  type MetaData,
  type PageData,
  type StaticSource,
} from "fumadocs-core/source";
import type { FileInfo } from "fumadocs-mdx/runtime/types";
// don't worry if this file is missing, we will run the dev command later to generate this file
import { actionDocs, cliDocs, docs, sourceDocs } from "./.source/server";

const docsSource = update(docs.toFumadocsSource())
  .page((page) => page)
  .build();

type PressPageData = PageData & {
  info: FileInfo;
};

type SourceConfig = {
  pageData: PressPageData;
  metaData: MetaData;
};

type SourceUpdater<T extends SourceConfig> = ReturnType<typeof update<T>>;
type UpdatedSource<T extends SourceConfig> = StaticSource<{
  pageData: T["pageData"];
  metaData: T["metaData"];
}>;

const docName = (path: string, pattern: RegExp, prefix = "") => {
  const [, name = path] = path.match(pattern) ?? [];

  return name.replace(prefix, "").replaceAll("_", "-");
};

const packageDocsSource = <T extends SourceConfig>(
  source: SourceUpdater<T>,
  filePattern: RegExp,
  slugPrefix: string,
  packagePrefix = "",
): UpdatedSource<T> =>
  source
    .files((files) => files.filter((file) => file.path.match(filePattern)))
    .page((page) => {
      const slugs = [slugPrefix, docName(page.path, filePattern, packagePrefix)];
      const path = slugs.join("/");
      const data: T["pageData"] = {
        ...page.data,
        info: {
          ...page.data.info,
          path: `/${path}`,
        },
      };

      return {
        ...page,
        slugs,
        path,
        data,
      };
    })
    .build();

const cliDocsSource = packageDocsSource(
  update(cliDocs.toFumadocsSource()),
  /^docs\/(.+)\.md$/,
  "cli",
);

const sourceDocsSource = packageDocsSource(
  update(sourceDocs.toFumadocsSource()),
  /^([^/]+)\/src\/docs\.md$/,
  "sources",
  "agentboard-source-",
);

const actionDocsSource = packageDocsSource(
  update(actionDocs.toFumadocsSource()),
  /^([^/]+)\/src\/docs\.md$/,
  "actions",
  "agentboard-action-",
);

export default defineConfig({
  content: {
    docs: docsSource,
    cliDocs: cliDocsSource,
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
