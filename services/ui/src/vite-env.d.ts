/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_ENABLE_PROBLEM_REPORT?: string;
  readonly VITE_PROBLEM_REPORT_EMAIL?: string;
  readonly VITE_PROBLEM_REPORT_GITHUB_REPO?: string;
  readonly VITE_BASE_PATH?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
