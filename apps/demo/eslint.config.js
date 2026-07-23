import { defineConfig } from "eslint/config";
import css from "@eslint/css";
import html from "@html-eslint/eslint-plugin";

export default defineConfig([
  {
    files: ["**/*.html"],
    plugins: { html },
    language: "html/html",
    rules: {
      "html/no-duplicate-class": "error",
      "html/require-img-alt": "error",
    },
  },
  {
    files: ["**/*.css"],
    plugins: { css },
    language: "css/css",
    rules: {
      "css/no-duplicate-imports": "error",
      "css/no-empty-blocks": "error",
      "css/no-invalid-at-rules": "error",
      "css/no-invalid-properties": "error",
    },
  },
]);