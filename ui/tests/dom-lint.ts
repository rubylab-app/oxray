import type { Page } from "@playwright/test";

export async function domLintProblems(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const problems: string[] = [];
    const sizes = new Set([11, 12, 13, 14, 16, 20]);
    const lines = new Set([16, 20, 24, 28]);
    const weights = new Set([400, 500, 600]);
    const tokenClasses = [
      "text-text",
      "text-text-2",
      "text-text-3",
      "text-accent",
      "text-accent-strong",
      "text-ok",
      "text-warn",
      "text-warn-strong",
      "text-err",
      "text-err-strong",
      "text-code-literal",
    ];
    const allowed = new Set<string>();
    for (const name of tokenClasses) {
      const probe = document.createElement("span");
      probe.className = name;
      document.body.append(probe);
      allowed.add(getComputedStyle(probe).color);
      probe.remove();
    }
    const elements = [document.body, ...document.body.querySelectorAll("*")];
    for (const element of elements) {
      if (
        element instanceof HTMLScriptElement ||
        element instanceof HTMLStyleElement ||
        element instanceof HTMLLinkElement
      ) {
        continue;
      }
      const style = getComputedStyle(element);
      const fontSize = Number.parseFloat(style.fontSize);
      const lineHeight = Number.parseFloat(style.lineHeight);
      const weight = Number.parseInt(style.fontWeight, 10);
      const where = elementLabel(element);
      if (!sizes.has(fontSize)) {
        problems.push(`font-size ${style.fontSize} on ${where}`);
      }
      if (!lines.has(lineHeight)) {
        problems.push(`line-height ${style.lineHeight} on ${where}`);
      }
      if (!weights.has(weight)) {
        problems.push(`font-weight ${style.fontWeight} on ${where}`);
      }
      if (
        style.boxShadow !== "none" &&
        element.getAttribute("data-floating") === null
      ) {
        problems.push(`box-shadow on ${where}`);
      }
      if (!allowed.has(style.color)) {
        problems.push(`color ${style.color} on ${where}`);
      }
      if (boxed(style) && hasBoxedAncestor(element)) {
        problems.push(`nested radius on ${where}`);
      }
    }
    const walker = document.createTreeWalker(
      document.body,
      NodeFilter.SHOW_TEXT,
    );
    let node = walker.nextNode();
    while (node) {
      const text = node.textContent?.replace(/\s+/g, " ").trim() ?? "";
      const parent = node.parentElement;
      if (text.length > 0 && parent && shown(parent)) {
        if (
          text === "undefined" ||
          text === "null" ||
          text === "NaN" ||
          text === "[object Object]" ||
          text === "N"
        ) {
          problems.push(`forbidden text ${text}`);
        }
        if (text.toLowerCase().includes("lorem")) {
          problems.push(`lorem in ${text}`);
        }
        if (
          /(^|[^\d])1\d{9}([^\d]|$)/.test(text) ||
          /(^|[^\d])1\d{12}([^\d]|$)/.test(text)
        ) {
          problems.push(`raw timestamp ${text}`);
        }
        if (/\p{Extended_Pictographic}/u.test(text)) {
          problems.push(`emoji ${text}`);
        }
      }
      node = walker.nextNode();
    }
    return problems.slice(0, 40);

    function elementLabel(element: Element): string {
      const name = element.tagName.toLowerCase();
      const label = element.getAttribute("aria-label");
      return label === null ? name : `${name} ${label}`;
    }

    function shown(element: Element): boolean {
      const style = getComputedStyle(element);
      if (style.display === "none" || style.visibility === "hidden") {
        return false;
      }
      const rect = element.getBoundingClientRect();
      return rect.width > 1 && rect.height > 1;
    }

    function boxed(style: CSSStyleDeclaration): boolean {
      const widths = [
        style.borderTopWidth,
        style.borderRightWidth,
        style.borderBottomWidth,
        style.borderLeftWidth,
      ].map((value) => Number.parseFloat(value));
      const colors = [
        style.borderTopColor,
        style.borderRightColor,
        style.borderBottomColor,
        style.borderLeftColor,
      ];
      const border = widths.some(
        (width, index) => width >= 1 && !clear(colors[index] ?? ""),
      );
      const radius = Math.max(
        Number.parseFloat(style.borderTopLeftRadius),
        Number.parseFloat(style.borderTopRightRadius),
        Number.parseFloat(style.borderBottomRightRadius),
        Number.parseFloat(style.borderBottomLeftRadius),
      );
      return border && radius >= 6;
    }

    function hasBoxedAncestor(element: Element): boolean {
      let parent = element.parentElement;
      while (parent) {
        if (boxed(getComputedStyle(parent))) {
          return true;
        }
        parent = parent.parentElement;
      }
      return false;
    }

    function clear(color: string): boolean {
      return color === "transparent" || color === "rgba(0, 0, 0, 0)";
    }
  });
}
