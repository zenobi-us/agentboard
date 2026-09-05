import { defineDocs } from "fumadocs-mdx/config";
import { metaSchema, pageSchema } from "fumapress/adapters/mdx/schema";

const docsOptions = {
  async: true,
  schema: pageSchema,
  postprocess: {
    includeProcessedMarkdown: true,
  },
} as const;

const metaOptions = {
  schema: metaSchema,
} as const;

export const docs = defineDocs({
  dir: "content",
  docs: docsOptions,
  meta: metaOptions,
});

export const cliDocs = defineDocs({
  dir: "../cli",
  docs: {
    files: ["docs/*.md"],
    ...docsOptions,
  },
  meta: metaOptions,
});

export const sourceDocs = defineDocs({
  dir: "../../pkgs/packages",
  docs: {
    files: ["clankpipe-source-*/src/docs.md"],
    ...docsOptions,
  },
  meta: {
    files: ["clankpipe-source-*/src/meta.json"],
    ...metaOptions,
  },
});

export const actionDocs = defineDocs({
  dir: "../../pkgs/packages",
  docs: {
    files: ["clankpipe-action-*/src/docs.md"],
    ...docsOptions,
  },
  meta: {
    files: ["clankpipe-action-*/src/meta.json"],
    ...metaOptions,
  },
});
