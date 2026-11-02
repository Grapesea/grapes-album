export function absoluteWorkspacePath(
  rootPath: string,
  relativePath: string,
): string {
  if (!relativePath) return rootPath;

  const separator = rootPath.includes("\\") ? "\\" : "/";
  const root = rootPath.replace(/[\\/]+$/, "");
  const relative = relativePath
    .split(/[\\/]+/)
    .filter(Boolean)
    .join(separator);
  return `${root}${separator}${relative}`;
}
