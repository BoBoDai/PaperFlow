export interface ArxivPaper {
  id: string;
  title: string;
  summary: string;
  authors: string[];
  published: string;
  categories: string[];
  pdfUrl: string;
  source: string;
  venue: string | null;
}

const API_BASE = 'http://localhost:8080';

export async function searchArxiv(query: string, maxResults: number = 5): Promise<ArxivPaper[]> {
  const url = `${API_BASE}/api/search?q=${encodeURIComponent(query)}&max_results=${maxResults}`;

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`API 错误: ${response.status}`);
  }

  const data = await response.json();
  return data.papers.map((p: any) => ({
    id: p.id,
    title: p.title,
    summary: p.summary,
    authors: p.authors,
    published: p.published,
    categories: p.categories,
    pdfUrl: p.pdf_url,
    source: p.source || 'arxiv',
    venue: p.venue || null,
  }));
}

/** Quick search by preset (robotics, ai, cv) */
export async function quickSearch(preset: string, maxResults: number = 10): Promise<ArxivPaper[]> {
  const url = `${API_BASE}/api/quick-search?preset=${encodeURIComponent(preset)}&max_results=${maxResults}`;

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`API 错误: ${response.status}`);
  }

  const data = await response.json();
  return data.papers.map((p: any) => ({
    id: p.id,
    title: p.title,
    summary: p.summary,
    authors: p.authors,
    published: p.published,
    categories: p.categories,
    pdfUrl: p.pdf_url,
    source: p.source || 'arxiv',
    venue: p.venue || null,
  }));
}

/** Save a paper to the database */
export async function savePaper(paper: ArxivPaper): Promise<void> {
  const url = `${API_BASE}/api/papers/save`;
  await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      id: paper.id,
      title: paper.title,
      authors: paper.authors,
      abstract_text: paper.summary,
      categories: paper.categories,
      published: paper.published,
      pdf_url: paper.pdfUrl,
      source: paper.source,
      venue: paper.venue,
    }),
  });
}

/** Search a single arXiv category (for streaming/parallel fetch) */
export async function searchByCategory(category: string, maxResults: number = 3): Promise<ArxivPaper[]> {
  const query = `cat:${category}`;
  const url = `${API_BASE}/api/search?q=${encodeURIComponent(query)}&max_results=${maxResults}`;

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`API 错误: ${response.status}`);
  }

  const data = await response.json();
  return data.papers.map((p: any) => ({
    id: p.id,
    title: p.title,
    summary: p.summary,
    authors: p.authors,
    published: p.published,
    categories: p.categories,
    pdfUrl: p.pdf_url,
    source: p.source || 'arxiv',
    venue: p.venue || null,
  }));
}

export async function listPapers(): Promise<ArxivPaper[]> {
  const url = `${API_BASE}/api/papers`;

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`API 错误: ${response.status}`);
  }

  const data = await response.json();
  return data.papers.map((p: any) => ({
    id: p.id,
    title: p.title,
    summary: p.summary,
    authors: p.authors,
    published: p.published,
    categories: p.categories,
    pdfUrl: p.pdf_url,
    source: p.source || 'arxiv',
    venue: p.venue || null,
  }));
}
