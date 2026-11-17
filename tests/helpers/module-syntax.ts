import * as fs from "./typescript-source.ts";
const { stripTypes } = fs;
import { maskCommentsAndStrings } from "./js-source.ts";

// prefix > 0 表示只剥掉声明的 export 前缀、保留声明本身（VM 里还得用这个符号）
type Declaration = { names: string[]; start: number; end: number; prefix: number } & ({ kind: "import"; specifier: string } | { kind: "export"; specifier: null });

// Extract static declarations for VM tests; product ESM is checked by its compiler.
function moduleDeclarations(source: string, file = "source.js") {
  source = stripTypes(source, file);
  const code = maskCommentsAndStrings(source);
  const declarations: Declaration[] = [];
  let depth = 0;
  for (let index = 0; index < code.length; index++) {
    if ("{([".includes(code[index])) depth++;
    if ("})]".includes(code[index])) depth--;
    const keyword = /^(import|export)\b/.exec(code.slice(index));
    if (!keyword || /[\w$.]/.test(code[index - 1] || "") || /^\s*:/.test(code.slice(index + keyword[0].length))) continue;
    if (depth !== 0) throw new Error("Module declaration must be top-level");
    const text = source.slice(index);
    const names = String.raw`[A-Za-z_$][\w$]*(?:\s*,\s*[A-Za-z_$][\w$]*)*\s*,?`;

    if (keyword[0] === "import") {
      const raw = /^import\s+(?:(?:\{\s*([^}]*?)\s*\}|([A-Za-z_$][\w$]*))\s+from\s+)?["']([^"'\r\n]+)["']\s*;/.exec(text);
      if (!raw) throw new Error(`Unsupported module syntax: ${text.slice(0, 80)}`);
      declarations.push({ kind: "import", names: (raw[1] ?? raw[2])?.split(",").map((name) => name.trim()).filter(Boolean) || [], specifier: raw[3], start: index, end: index + raw[0].length, prefix: 0 } as Declaration);
      index += raw[0].length - 1;
      continue;
    }

    // export { A, B }; / export { A } from "./x.ts"; / export * from "./x.ts"; 是纯导出语句，整句可以剥掉
    const list = new RegExp(String.raw`^export\s*(?:\{\s*(${names})?\s*\}|\*)\s*(?:from\s*["'][^"']*["']\s*)?;`).exec(text);
    if (list) {
      declarations.push({ kind: "export", names: list[1]?.split(",").map((name) => name.trim()).filter(Boolean) || [], specifier: null, start: index, end: index + list[0].length, prefix: 0 } as Declaration);
      index += list[0].length - 1;
      continue;
    }

    // export function / export class：带声明，只剥 export 前缀，声明留给 VM
    const declared = /^export\s+(?:async\s+)?(?:function|class)\s+[A-Za-z_$][\w$]*/.exec(text);
    if (declared) {
      const prefix = /^export\s+/.exec(text)![0].length;
      declarations.push({ kind: "export", names: [], specifier: null, start: index, end: index + declared[0].length, prefix } as Declaration);
      index += declared[0].length - 1;
      continue;
    }

    // export const X = ...; 同上，只剥 export 前缀
    const binding = /^export\s+(?:const|let|var)\s+([A-Za-z_$][\w$]*)/.exec(text);
    if (binding) {
      const prefix = /^export\s+/.exec(text)![0].length;
      declarations.push({ kind: "export", names: [binding[1]], specifier: null, start: index, end: index + binding[0].length, prefix } as Declaration);
      index += binding[0].length - 1;
      continue;
    }

    throw new Error(`Unsupported module syntax: ${text.slice(0, 80)}`);
  }
  return declarations;
}

function stripModuleSyntax(source: string, file = "source.js") {
  source = stripTypes(source, file);
  let result = source;
  for (const declaration of moduleDeclarations(source, "source.js").reverse()) {
    const piece = source.slice(declaration.start, declaration.end);
    const blanked = declaration.prefix > 0
      ? piece.slice(0, declaration.prefix).replace(/[^\r\n]/g, "") + piece.slice(declaration.prefix)
      : piece.replace(/[^\r\n]/g, "");
    result = result.slice(0, declaration.start) + blanked + result.slice(declaration.end);
  }
  return result;
}

function readFileSync(file: string, options: BufferEncoding) {
  const source = fs.readFileSync(file, options);
  return /\.(?:js|ts)$/.test(String(file)) ? stripModuleSyntax(source, "source.js") : source;
}

export { moduleDeclarations, stripModuleSyntax, readFileSync };

export type Contract = { moduleDeclarations: typeof moduleDeclarations; stripModuleSyntax: typeof stripModuleSyntax; readFileSync: typeof readFileSync };
