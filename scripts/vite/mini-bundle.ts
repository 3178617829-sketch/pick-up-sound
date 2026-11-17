import assert from "node:assert/strict";
import path from "node:path";
import { build, type Plugin, type ResolvedConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { preserveHtmlScripts } from "./preserve-html.ts";
import { vueFeatureFlags } from "../vue/features.ts";

/** 主窗口之外，需要单独打一份自包含产物的辅助窗口（迷你窗、任务栏条）。 */
const SECONDARY_ENTRIES: Record<string, string> = {
  mini: "mini.html",
  bar: "bar.html",
};

export function miniWindowBundle(): Plugin {
  let config: ResolvedConfig;
  return {
    name: "isolated-mini-window",
    apply: "build",
    configResolved(value) { config = value; },
    async generateBundle(_options, bundle) {
      for (const item of Object.values(bundle)) if (item.type === "chunk") {
        assert.equal(item.imports.length + item.dynamicImports.length, 0, "Main bundle must be self-contained");
      }
      for (const [name, file] of Object.entries(SECONDARY_ENTRIES)) {
        const built = await build({ configFile: false, root: config.root, base: config.base, publicDir: false,
          logLevel: "silent", plugins: [preserveHtmlScripts(), vue()], define: vueFeatureFlags,
          build: { write: false, modulePreload: false, minify: config.build.minify, target: config.build.target, sourcemap: false,
            rolldownOptions: { input: { [name]: path.join(config.root, file) }, output: { codeSplitting: false } } },
        });
        assert.ok(!Array.isArray(built) && "output" in built, `Unexpected ${name} build output`);
        for (const item of built.output) {
          if (item.type === "chunk") {
            assert.equal(item.imports.length + item.dynamicImports.length, 0, `${name} bundle must be self-contained`);
            this.emitFile({ type: "asset", fileName: item.fileName, source: item.code });
          } else this.emitFile({ type: "asset", fileName: item.fileName, source: item.source });
        }
      }
    },
  };
}
