import { defineConfig } from "waku/config";
import tailwindcss from "@tailwindcss/vite";
import press from "fumapress/vite";
import mdx from "fumadocs-mdx/vite";

export default defineConfig({
  basePath: "/clankpipe/",
  vite: {
    environments: {
      rsc: {
        resolve: {
          noExternal: true,
        },
      },
    },
    plugins: [press(), mdx(), tailwindcss()],
  },
});
