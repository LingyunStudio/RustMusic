import { marked } from "marked";
import DOMPurify from "dompurify";

/** GitHub release notes（markdown）→ 安全 HTML（React 以 dangerouslySetInnerHTML 渲染，
    样式见 index.css 的 .release-notes） */
export function renderMarkdown(md: string): string {
  const html = marked.parse(md ?? "", {
    async: false,
    gfm: true,
    breaks: true,
  }) as string;
  return DOMPurify.sanitize(html);
}
