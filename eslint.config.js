import js from "@eslint/js";
import pluginVue from "eslint-plugin-vue";
import tsEslint from "typescript-eslint";
import prettierConfig from "eslint-config-prettier";

export default [
  js.configs.recommended,
  ...tsEslint.configs.recommended,
  ...pluginVue.configs["flat/recommended"],
  prettierConfig,
  {
    files: ["src/**/*.{ts,vue}"],
    languageOptions: {
      parserOptions: {
        parser: tsEslint.parser,
        extraFileExtensions: [".vue"],
      },
      ecmaVersion: "latest",
      sourceType: "module",
      globals: {
        window: "readonly",
        document: "readonly",
        navigator: "readonly",
        console: "readonly",
        setTimeout: "readonly",
        clearTimeout: "readonly",
        setInterval: "readonly",
        clearInterval: "readonly",
        requestAnimationFrame: "readonly",
        fetch: "readonly",
        localStorage: "readonly",
      },
    },
    rules: {
      "no-undef": "off",
      "no-console": "warn",
      "vue/multi-word-component-names": "off",
      "vue/require-default-prop": "off",
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_" },
      ],
      "@typescript-eslint/no-explicit-any": "warn",
    },
  },
  {
    ignores: [
      "dist/**",
      "src-tauri/target/**",
      "node_modules/**",
      "*.config.js",
      "eslint.config.js",
    ],
  },
];
