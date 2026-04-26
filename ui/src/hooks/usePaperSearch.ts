import { useState, useCallback } from 'react';
import { ArxivPaper, searchArxiv } from '../services/arxiv';

interface UsePaperSearchReturn {
  papers: ArxivPaper[];
  isLoading: boolean;
  error: string | null;
  search: (query: string, maxResults?: number) => Promise<ArxivPaper[]>;
}

export const usePaperSearch = (): UsePaperSearchReturn => {
  const [papers, setPapers] = useState<ArxivPaper[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const search = useCallback(async (query: string, maxResults: number = 5): Promise<ArxivPaper[]> => {
    setIsLoading(true);
    setError(null);

    try {
      const results = await searchArxiv(query, maxResults);
      setPapers(results);
      return results;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      throw err;
    } finally {
      setIsLoading(false);
    }
  }, []);

  return { papers, isLoading, error, search };
};
