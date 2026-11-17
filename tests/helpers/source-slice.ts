import { createRequire } from "node:module";
import { isTypeScript, eraseTypes } from "./typescript-source.ts";



function sourceSlice(source: string, file: string, startMarker: string | null, endMarker: string | null, {
  endAfterStart = false,
  startLastBefore = null,
  includeEnd = false,
  stripModules = true,
}: { endAfterStart?: boolean; startLastBefore?: string | null; includeEnd?: boolean; stripModules?: boolean } = {}) {
  const erased = eraseTypes(source, file);
  if (isTypeScript(file) && stripModules) {
    source = createRequire(import.meta.url)("./module-syntax.ts").stripModuleSyntax(source, file);
  } else {
    source = erased;
  }
  const find = (marker: string, from = 0) => {
    const index = source.indexOf(marker, from);
    if (index < 0) throw new Error(`${file}: marker not found: ${JSON.stringify(marker)}`);
    return index;
  };

  const start = startMarker === null
    ? 0
    : startLastBefore === null
      ? find(startMarker)
      : source.lastIndexOf(startMarker, find(startLastBefore));
  if (start < 0) throw new Error(`${file}: marker not found: ${JSON.stringify(startMarker)}`);
  // endMarker 传 null = 取到文件末尾（源文件尾部没有可用锚点时用）
  const end = endMarker === null ? source.length : find(endMarker, endAfterStart ? start : 0);
  if (endMarker !== null && end <= start) {
    throw new Error(`${file}: end marker ${JSON.stringify(endMarker)} is not after start marker ${JSON.stringify(startMarker)}`);
  }
  const slice = source.slice(start, end + (includeEnd && endMarker !== null ? endMarker.length : 0));
  return slice;
}

export { sourceSlice };

export type Contract = { sourceSlice: typeof sourceSlice };
