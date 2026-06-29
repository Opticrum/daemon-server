import js from "@eslint/js"
import tseslint from "typescript-eslint"
import pluginVue from "eslint-plugin-vue"

export default [
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs["flat/recommended"],
  {
    files: ["**/*.{ts,vue}"],
    languageOptions: {
      parserOptions: { parser: "@typescript-eslint/parser" },
    },
    rules: {
      "@typescript-eslint/no-explicit-any": "off",
      "no-undef": "off", // handled by TypeScript
      "vue/multi-word-component-names": "off",
    },
  },
]
