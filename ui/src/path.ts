export function captureIdFromPath(pathname: string): string | null {
  const match = /\/r\/([^/]+)\/?$/.exec(pathname);
  const id = match?.[1];
  if (id === undefined || id.length === 0) {
    return null;
  }
  return decodeURIComponent(id);
}

export function listPathname(pathname: string): string {
  const stripped = pathname.replace(/\/r\/[^/]+\/?$/, "");
  if (stripped === "" || stripped === "/") {
    return "/";
  }
  return stripped.endsWith("/") ? stripped : `${stripped}/`;
}
