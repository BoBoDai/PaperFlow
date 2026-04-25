-- Initial migration

CREATE TABLE IF NOT EXISTS papers (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    authors TEXT NOT NULL,
    abstract_text TEXT NOT NULL,
    categories TEXT NOT NULL,
    published INTEGER NOT NULL,
    updated INTEGER NOT NULL,
    pdf_url TEXT NOT NULL,
    relevance_score REAL,
    is_read INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS summaries (
    id TEXT PRIMARY KEY,
    paper_id TEXT NOT NULL,
    short_summary TEXT NOT NULL,
    detailed_summary TEXT NOT NULL,
    key_points TEXT NOT NULL,
    generated_at INTEGER NOT NULL,
    provider TEXT NOT NULL,
    FOREIGN KEY (paper_id) REFERENCES papers(id)
);

CREATE TABLE IF NOT EXISTS preferences (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    keywords TEXT NOT NULL,
    categories TEXT NOT NULL,
    exclude_categories TEXT NOT NULL,
    fetch_interval_minutes INTEGER NOT NULL DEFAULT 60,
    max_papers_per_fetch INTEGER NOT NULL DEFAULT 5,
    voice_speed REAL NOT NULL DEFAULT 1.0,
    voice_volume REAL NOT NULL DEFAULT 1.0,
    llm_provider TEXT NOT NULL DEFAULT 'minimax',
    speech_provider TEXT NOT NULL DEFAULT 'groq_whisper',
    synthesizer_provider TEXT NOT NULL DEFAULT 'system_say'
);

CREATE INDEX IF NOT EXISTS idx_papers_published ON papers(published DESC);
CREATE INDEX IF NOT EXISTS idx_papers_is_read ON papers(is_read);
