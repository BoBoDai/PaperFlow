import type { PDFDocumentProxy } from 'pdfjs-dist';

export const MAX_EXTRACT_CHARS = 200_000;
export const MAX_EXTRACT_PAGES = 400;

interface PdfTextItem {
  str?: string;
  hasEOL?: boolean;
}

export function extractPageText(items: PdfTextItem[]): string {
  return items
    .map((item) => (item.str ?? '') + (item.hasEOL ? '\n' : ''))
    .join(' ')
    .replace(/[ \t]+/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

export function pageTag(page: number, text: string): string {
  return `\n\n[第 ${page} 页]\n${text}`;
}

export async function extractPage(
  doc: PDFDocumentProxy,
  page: number,
): Promise<string> {
  const pdfPage = await doc.getPage(page);
  const content = await pdfPage.getTextContent();
  return extractPageText(content.items as unknown as PdfTextItem[]);
}

export async function extractFullText(
  doc: PDFDocumentProxy,
  onProgress?: (done: number, total: number) => void,
): Promise<string> {
  const total = Math.min(doc.numPages, MAX_EXTRACT_PAGES);
  let text = '';

  for (let page = 1; page <= total; page += 1) {
    if (text.length >= MAX_EXTRACT_CHARS) break;

    const pageText = await extractPage(doc, page);
    text += pageTag(page, pageText);
    onProgress?.(page, total);

    if (page % 5 === 0) {
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
  }

  return text.trim();
}
