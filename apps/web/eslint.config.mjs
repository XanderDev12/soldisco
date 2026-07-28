import { defineConfig, globalIgnores } from "eslint/config";
import tsPlugin from "@typescript-eslint/eslint-plugin";
import reactHooks from "eslint-plugin-react-hooks";

export default defineConfig([
  globalIgnores([
    ".next/**",
    "apps/web/.next/**",
    ".open-next/**",
    "apps/web/.open-next/**",
    ".wrangler/**",
    "apps/web/.wrangler/**",
    "dist/**",
    "apps/web/dist/**",
    "node_modules/**",
    "out/**",
    "next-env.d.ts",
  ]),
  ...tsPlugin.configs["flat/recommended"],
  {
    ...reactHooks.configs.flat.recommended,
    files: ["**/*.{ts,tsx}"],
  },
]);
