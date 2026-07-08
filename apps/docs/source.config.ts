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

export const sourceDocs = defineDocs({
  dir: "../../pkgs/crates/agentboard/src",
  docs: {
    files: ["sources/*/docs.md"],
    ...docsOptions,
  },
  meta: metaOptions,
});

export const actionDocs = defineDocs({
  dir: "../../pkgs/crates/agentboard/src",
  docs: {
    files: ["actions/*/docs.md"],
    ...docsOptions,
  },
  meta: metaOptions,
});
