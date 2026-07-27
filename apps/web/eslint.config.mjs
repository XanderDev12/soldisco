import { defineConfig, globalIgnores } from "eslint/config";
import tsPlugin from "@typescript-eslint/eslint-plugin";
import reactHooks from "eslint-plugin-react-hooks";

export default defineConfig([
  globalIgnores([
    ".next/**",
    ".open-next/**",
    ".wrangler/**",
    "dist/**",
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
