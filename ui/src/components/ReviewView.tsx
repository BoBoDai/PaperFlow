import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  downloadPdf,
  finalizeOpinions,
  personaChatStream,
  reviewPaper,
} from '../api/ipc';
import { ipcErrorMessage } from '../lib/ipcError';
import { PDF_DOCUMENT_OPTIONS, pdfjs } from '../lib/pdfjs';
import { extractFullText } from '../lib/pdfText';
import type {
  MagiTurn,
  OpinionBatch,
  OpinionOutcome,
  Paper,
  PersonaId,
  PersonaOpinion,
  PersonaOutcome,
  PersonaReview,
  ReviewResult,
  ReviewVerdict,
  Stance,
} from '../types';

interface ReviewViewProps {
  paper: Paper;
  onClose: () => void;
}

const PERSONAS: PersonaId[] = ['rigor', 'repro', 'insight'];

const PERSONA_META: Record<
  PersonaId,
  { label: string; unit: string; lens: string }
> = {
  rigor: { label: '严谨', unit: 'UNIT 01', lens: '方法与证据' },
  repro: { label: '复现', unit: 'UNIT 02', lens: '可复现性' },
  insight: { label: '洞见', unit: 'UNIT 03', lens: '新颖性与价值' },
};

const STANCE_LABEL: Record<Stance, string> = {
  support: 'SUPPORT',
  borderline: 'BORDERLINE',
  oppose: 'OPPOSE',
};

const STANCE_CLASS: Record<Stance, string> = {
  support: 'border-term-cyan text-term-cyan',
  borderline: 'border-term-amber text-term-amber',
  oppose: 'border-term-red text-term-red',
};

const STANCE_TEXT: Record<Stance, string> = {
  support: 'text-term-cyan',
  borderline: 'text-term-amber',
  oppose: 'text-term-red',
};

const STANCE_BAR: Record<Stance, string> = {
  support: 'bg-term-cyan',
  borderline: 'bg-term-amber',
  oppose: 'bg-term-red',
};

const STANCE_LED: Record<Stance, string> = {
  support: 'bg-term-cyan',
  borderline: 'bg-term-amber',
  oppose: 'bg-term-red',
};

function emptyStreaming(): Record<PersonaId, string> {
  return { rigor: '', repro: '', insight: '' };
}

function stanceLabel(stance: Stance): string {
  return STANCE_LABEL[stance];
}

function verdictLabel(verdict: ReviewVerdict): string {
  switch (verdict.kind) {
    case 'consensus':
      return `CONSENSUS 3:0 · ${stanceLabel(verdict.stance)}`;
    case 'majority':
      return `MAJORITY 2:1 · ${stanceLabel(verdict.stance)}`;
    case 'split':
      return 'SPLIT · 需要你自己读';
    case 'incomplete':
      return 'INCOMPLETE · 有脑未完成';
  }
}

function findOutcome(
  result: ReviewResult | null,
  persona: PersonaId,
): PersonaOutcome | undefined {
  return result?.outcomes.find((outcome) =>
    outcome.status === 'ok'
      ? outcome.review.persona === persona
      : outcome.persona === persona,
  );
}

function findReview(
  result: ReviewResult | null,
  persona: PersonaId,
): PersonaReview | null {
  const outcome = findOutcome(result, persona);
  return outcome?.status === 'ok' ? outcome.review : null;
}

function findOpinion(
  batch: OpinionBatch | null,
  persona: PersonaId,
): OpinionOutcome | undefined {
  return batch?.outcomes.find((outcome) =>
    outcome.status === 'ok'
      ? outcome.opinion.persona === persona
      : outcome.persona === persona,
  );
}

function MicroLabel({ children }: { children: string }) {
  return (
    <div className="text-[9.5px] uppercase tracking-[0.22em] text-term-dim">
      {children}
    </div>
  );
}

function ErrorBlock({ message }: { message: string }) {
  return (
    <div className="mt-2 whitespace-pre-wrap border border-term-red px-2 py-1.5 text-[11px] text-term-red">
      {message}
    </div>
  );
}

function BulletList({ items, empty }: { items: string[]; empty: string }) {
  if (items.length === 0) {
    return <p className="text-[11px] text-term-dim">{empty}</p>;
  }

  return (
    <ul className="m-0 list-none space-y-0.5 p-0 text-[11.5px]">
      {items.map((item, index) => (
        <li key={`${index}-${item}`} className="flex gap-1.5">
          <span className="text-term-dim">›</span>
          <span className="text-term-fg">{item}</span>
        </li>
      ))}
    </ul>
  );
}

function ReviewBody({ review }: { review: PersonaReview }) {
  return (
    <div className="mt-2 space-y-2.5 text-[12px] leading-relaxed">
      <div className="flex items-end justify-between gap-2">
        <span
          className={`border px-1.5 py-0.5 text-[9.5px] tracking-[0.16em] ${STANCE_CLASS[review.stance]}`}
        >
          {stanceLabel(review.stance)}
        </span>
        <span className="text-2xl leading-none">{review.confidence}</span>
      </div>
      <div className="h-1 w-full bg-term-grid">
        <div
          className={`h-full ${STANCE_BAR[review.stance]}`}
          style={{ width: `${review.confidence}%` }}
        />
      </div>
      <p className="m-0 whitespace-pre-wrap text-term-fg">{review.summary}</p>
      <div>
        <MicroLabel>STRENGTHS</MicroLabel>
        <BulletList items={review.strengths} empty="—" />
      </div>
      <div>
        <MicroLabel>CONCERNS</MicroLabel>
        <BulletList items={review.concerns} empty="—" />
      </div>
      <div>
        <MicroLabel>RISK</MicroLabel>
        <p className="m-0 text-term-fg">{review.riskiest_claim}</p>
      </div>
      <div>
        <MicroLabel>CHALLENGE</MicroLabel>
        <p className="m-0 text-term-dim">{review.would_change_mind}</p>
      </div>
    </div>
  );
}

function OpinionBlock({ opinion }: { opinion: PersonaOpinion }) {
  return (
    <div className="mt-3 border-t border-term-grid pt-2">
      <MicroLabel>FINAL OPINION</MicroLabel>
      <div className="mt-1 flex items-center gap-2">
        <span
          className={`border px-1.5 py-0.5 text-[9.5px] tracking-[0.16em] ${STANCE_CLASS[opinion.stance]}`}
        >
          {stanceLabel(opinion.stance)}
        </span>
        <span className="text-lg leading-none">{opinion.confidence}</span>
      </div>
      <p className="mt-1 whitespace-pre-wrap text-[11.5px] text-term-fg">
        {opinion.reason}
      </p>
    </div>
  );
}

export default function ReviewView({ paper, onClose }: ReviewViewProps) {
  const [review, setReview] = useState<ReviewResult | null>(null);
  const [reviewBusy, setReviewBusy] = useState(true);
  const [reviewError, setReviewError] = useState<string | null>(null);

  const [context, setContext] = useState<'abstract' | 'full_text'>('abstract');
  const [fullText, setFullText] = useState<string | null>(null);
  const [fullTextBusy, setFullTextBusy] = useState(false);
  const [fullTextProgress, setFullTextProgress] = useState<{
    done: number;
    total: number;
  } | null>(null);
  const [fullTextError, setFullTextError] = useState<string | null>(null);

  const [turns, setTurns] = useState<MagiTurn[]>([]);
  const [draft, setDraft] = useState('');
  const [chatBusy, setChatBusy] = useState(false);
  const [activePersona, setActivePersona] = useState<PersonaId | null>(null);
  const [streaming, setStreaming] =
    useState<Record<PersonaId, string>>(emptyStreaming);
  const [personaErrors, setPersonaErrors] = useState<
    Partial<Record<PersonaId, string>>
  >({});

  const [batch, setBatch] = useState<OpinionBatch | null>(null);
  const [finalizing, setFinalizing] = useState(false);
  const [finalizeError, setFinalizeError] = useState<string | null>(null);

  const reviewRequestId = useRef(0);
  const chatRequestId = useRef(0);
  const startedRef = useRef(false);
  const endRef = useRef<HTMLDivElement | null>(null);

  const contextText = useMemo(
    () => (fullText ?? paper.abstract_text).trim(),
    [fullText, paper.abstract_text],
  );

  const panelReviews = useMemo(
    () =>
      PERSONAS.map((persona) => findReview(review, persona)).filter(
        (item): item is PersonaReview => item !== null,
      ),
    [review],
  );

  const runReview = useCallback(
    async (fullTextValue?: string) => {
      const requestId = ++reviewRequestId.current;
      setReviewBusy(true);
      setReviewError(null);
      setBatch(null);
      setFinalizeError(null);
      setTurns([]);
      setStreaming(emptyStreaming());
      setPersonaErrors({});

      try {
        const result = await reviewPaper({
          paper,
          context: fullTextValue ?? null,
        });
        if (requestId !== reviewRequestId.current) return;
        setReview(result);
        setContext(result.context);
      } catch (error: unknown) {
        if (requestId === reviewRequestId.current) {
          setReviewError(ipcErrorMessage(error));
        }
      } finally {
        if (requestId === reviewRequestId.current) setReviewBusy(false);
      }
    },
    [paper],
  );

  useEffect(() => {
    if (startedRef.current) return;
    startedRef.current = true;
    void runReview();
  }, [runReview]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [onClose]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: 'end' });
  }, [turns, streaming, activePersona]);

  const loadFullText = useCallback(async () => {
    if (!paper.pdf_url || fullTextBusy) return;

    setFullTextBusy(true);
    setFullTextError(null);
    setFullTextProgress(null);
    let loadingTask: ReturnType<typeof pdfjs.getDocument> | null = null;

    try {
      const buffer = await downloadPdf(paper.pdf_url);
      loadingTask = pdfjs.getDocument({
        data: new Uint8Array(buffer),
        ...PDF_DOCUMENT_OPTIONS,
      });
      const doc = await loadingTask.promise;
      const text = await extractFullText(doc, (done, total) => {
        setFullTextProgress({ done, total });
      });
      setFullText(text);
      await runReview(text);
    } catch (error: unknown) {
      setFullTextError(ipcErrorMessage(error));
    } finally {
      await loadingTask?.destroy();
      setFullTextBusy(false);
      setFullTextProgress(null);
    }
  }, [paper.pdf_url, fullTextBusy, runReview]);

  const runRound = useCallback(
    async (question: string, baseTranscript: MagiTurn[]) => {
      setChatBusy(true);
      setBatch(null);
      setFinalizeError(null);
      setPersonaErrors({});

      const requestId = ++chatRequestId.current;
      let transcript = baseTranscript;

      for (const persona of PERSONAS) {
        if (requestId !== chatRequestId.current) return;

        setActivePersona(persona);
        setStreaming((current) => ({ ...current, [persona]: '' }));

        try {
          const answer = await personaChatStream(
            {
              paper,
              persona,
              question,
              context: contextText,
              transcript,
              own_review:
                panelReviews.find((item) => item.persona === persona) ??
                null,
              panel_reviews: panelReviews,
            },
            (delta) => {
              if (requestId !== chatRequestId.current) return;
              setStreaming((current) => ({
                ...current,
                [persona]: current[persona] + delta,
              }));
            },
          );

          if (requestId !== chatRequestId.current) return;
          transcript = [
            ...transcript,
            { persona, content: answer.answer },
          ];
          setTurns(transcript);
          setStreaming((current) => ({ ...current, [persona]: '' }));
        } catch (error: unknown) {
          if (requestId !== chatRequestId.current) return;
          setPersonaErrors((current) => ({
            ...current,
            [persona]: ipcErrorMessage(error),
          }));
          setStreaming((current) => ({ ...current, [persona]: '' }));
        }
      }

      if (requestId === chatRequestId.current) {
        setActivePersona(null);
        setChatBusy(false);
      }
    },
    [paper, contextText, panelReviews],
  );

  const send = useCallback(() => {
    const question = draft.trim();
    if (!question || chatBusy || reviewBusy || !contextText) return;

    setDraft('');
    const base = [...turns, { persona: null, content: question } as MagiTurn];
    setTurns(base);
    void runRound(question, base);
  }, [draft, chatBusy, reviewBusy, contextText, turns, runRound]);

  const retryRound = useCallback(() => {
    if (chatBusy) return;

    const lastUserIndex = turns.map((turn) => turn.persona).lastIndexOf(null);
    if (lastUserIndex < 0) return;

    const question = turns[lastUserIndex]?.content;
    if (question === undefined) return;
    const base = turns.slice(0, lastUserIndex + 1);
    setTurns(base);
    void runRound(question, base);
  }, [chatBusy, turns, runRound]);

  const finalize = useCallback(async () => {
    if (finalizing || chatBusy || turns.length === 0 || !contextText) return;

    setFinalizing(true);
    setFinalizeError(null);

    try {
      const lastQuestion =
        [...turns].reverse().find((turn) => turn.persona === null)?.content ??
        '请基于以上讨论给出你对这篇论文的最终判断。';
      const result = await finalizeOpinions({
        paper,
        question: lastQuestion,
        transcript: turns,
        context: contextText,
        reviews: panelReviews,
      });
      setBatch(result);
    } catch (error: unknown) {
      setFinalizeError(ipcErrorMessage(error));
    } finally {
      setFinalizing(false);
    }
  }, [
    finalizing,
    chatBusy,
    turns,
    contextText,
    panelReviews,
    paper,
  ]);

  const verdict = batch?.verdict ?? review?.verdict ?? null;
  const hasChatError = PERSONAS.some((persona) => personaErrors[persona]);

  return (
    <div
      className="fixed inset-0 z-20 flex flex-col bg-term-bg font-mono text-term-fg"
      role="dialog"
      aria-modal="true"
      aria-label="圆桌评审"
    >
      <div className="magi-scanlines pointer-events-none fixed inset-0 z-30" />

      <header className="relative z-10 flex items-center gap-3 border-b border-term-grid px-4 py-2.5">
        <div className="min-w-0 flex-1">
          <div className="text-[9.5px] uppercase tracking-[0.26em] text-term-dim">
            PaperFlow // Round-Table Review
          </div>
          <div className="truncate text-sm">{paper.title}</div>
        </div>
        <span
          className={`border px-2 py-0.5 text-[9.5px] tracking-[0.18em] ${
            context === 'full_text'
              ? 'border-term-cyan text-term-cyan'
              : 'border-term-amber text-term-amber'
          }`}
        >
          {context === 'full_text' ? 'FULL TEXT' : 'ABSTRACT'}
        </span>
        <button
          className="px-2 py-1 text-base text-term-dim hover:text-term-fg"
          onClick={onClose}
          aria-label="关闭圆桌评审"
        >
          ✕
        </button>
      </header>

      <div className="relative z-10 flex min-h-0 flex-1 flex-col">
        <div className="grid grid-cols-3 border-b border-term-grid max-[900px]:grid-cols-1">
          {PERSONAS.map((persona) => {
            const meta = PERSONA_META[persona];
            const outcome = findOutcome(review, persona);
            const opinionOutcome = findOpinion(batch, persona);
            const opinion =
              opinionOutcome?.status === 'ok'
                ? opinionOutcome.opinion
                : null;
            const stance =
              opinion?.stance ??
              (outcome?.status === 'ok' ? outcome.review.stance : null);
            const confidence =
              opinion?.confidence ??
              (outcome?.status === 'ok' ? outcome.review.confidence : null);
            const failed =
              outcome?.status === 'failed' ||
              opinionOutcome?.status === 'failed';
            const busy = activePersona === persona;

            const ledClass = failed
              ? 'bg-term-red'
              : busy
                ? 'bg-term-amber animate-pulse'
                : stance
                  ? STANCE_LED[stance]
                  : 'bg-term-grid';

            return (
              <div
                key={persona}
                className="flex items-center gap-2 border-r border-term-grid px-3 py-1.5 last:border-r-0 max-[900px]:border-r-0 max-[900px]:border-b"
              >
                <span className={`h-1.5 w-1.5 ${ledClass}`} />
                <span className="text-[9.5px] tracking-[0.16em] text-term-dim">
                  {meta.unit} · {meta.label}
                </span>
                <span
                  className={`ml-auto text-[9.5px] tracking-[0.12em] ${
                    stance ? STANCE_TEXT[stance] : 'text-term-dim'
                  }`}
                >
                  {stance
                    ? stanceLabel(stance)
                    : failed
                      ? 'FAILED'
                      : 'PENDING'}
                </span>
                <span className="w-8 text-right text-lg leading-none">
                  {confidence ?? '—'}
                </span>
              </div>
            );
          })}
        </div>

        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-term-grid px-4 py-1.5">
          <span className="text-[9.5px] uppercase tracking-[0.22em] text-term-fg">
            VERDICT // {verdict ? verdictLabel(verdict) : 'PENDING'}
          </span>
          <span className="text-[9.5px] tracking-[0.16em] text-term-dim">
            {batch ? 'FINAL BALLOT' : review ? 'INITIAL REVIEW' : 'ANALYZING'}
          </span>
          {context === 'abstract' && (
            <span className="text-[9.5px] tracking-[0.16em] text-term-amber">
              CONFIDENCE ≤ 70
            </span>
          )}
          {context === 'abstract' && paper.pdf_url && (
            <button
              className="ml-auto border border-term-grid px-2 py-0.5 text-[9.5px] tracking-[0.14em] text-term-dim hover:border-term-cyan hover:text-term-cyan disabled:opacity-40"
              onClick={() => void loadFullText()}
              disabled={fullTextBusy}
            >
              {fullTextBusy
                ? `EXTRACTING ${fullTextProgress ? `${fullTextProgress.done}/${fullTextProgress.total}` : ''}`
                : '加载全文重评'}
            </button>
          )}
          {reviewBusy && (
            <button
              className="ml-auto border border-term-grid px-2 py-0.5 text-[9.5px] tracking-[0.14em] text-term-dim"
              disabled
            >
              ANALYZING…
            </button>
          )}
          {reviewError && (
            <button
              className="ml-auto border border-term-red px-2 py-0.5 text-[9.5px] tracking-[0.14em] text-term-red"
              onClick={() => void runReview(fullText ?? undefined)}
            >
              重新评审
            </button>
          )}
        </div>

        {reviewError && (
          <div className="mx-4 mt-2">
            <ErrorBlock message={reviewError} />
          </div>
        )}
        {fullTextError && (
          <div className="mx-4 mt-2">
            <ErrorBlock message={fullTextError} />
          </div>
        )}

        <div className="grid max-h-[42vh] min-h-0 grid-cols-3 overflow-y-auto border-b border-term-grid max-[900px]:max-h-[50vh] max-[900px]:grid-cols-1">
          {PERSONAS.map((persona) => {
            const meta = PERSONA_META[persona];
            const outcome = findOutcome(review, persona);
            const opinionOutcome = findOpinion(batch, persona);
            const opinion =
              opinionOutcome?.status === 'ok'
                ? opinionOutcome.opinion
                : null;
            const opinionError =
              opinionOutcome?.status === 'failed'
                ? opinionOutcome.message
                : null;

            return (
              <section
                key={persona}
                className="min-w-0 border-r border-term-grid p-3 last:border-r-0 max-[900px]:border-r-0 max-[900px]:border-b"
              >
                <div className="flex items-center gap-2">
                  <span className="text-[10px] tracking-[0.16em] text-term-fg">
                    {meta.label}
                  </span>
                  <span className="text-[9px] tracking-[0.16em] text-term-dim">
                    {meta.lens}
                  </span>
                  <span
                    className={`ml-auto h-1.5 w-1.5 ${
                      outcome?.status === 'failed'
                        ? 'bg-term-red'
                        : activePersona === persona
                          ? 'bg-term-amber animate-pulse'
                          : outcome?.status === 'ok'
                            ? STANCE_LED[outcome.review.stance]
                            : 'bg-term-grid'
                    }`}
                  />
                </div>

                {reviewBusy && !review && (
                  <p className="mt-4 animate-pulse text-[11px] text-term-dim">
                    ANALYZING…
                  </p>
                )}
                {outcome?.status === 'failed' && (
                  <ErrorBlock message={outcome.message} />
                )}
                {outcome?.status === 'ok' && (
                  <ReviewBody review={outcome.review} />
                )}
                {opinion && <OpinionBlock opinion={opinion} />}
                {opinionError && <ErrorBlock message={opinionError} />}
                {outcome?.status === 'ok' && (
                  <p className="mt-2 text-[9px] tracking-[0.14em] text-term-dim">
                    MODEL {outcome.review.model || '—'}
                  </p>
                )}
              </section>
            );
          })}
        </div>

        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-4 py-3">
          <div className="mb-2 flex items-center gap-2">
            <MicroLabel>GROUP CHAT</MicroLabel>
            {hasChatError && (
              <button
                className="ml-auto border border-term-red px-2 py-0.5 text-[9px] tracking-[0.14em] text-term-red"
                onClick={retryRound}
                disabled={chatBusy}
              >
                重试本轮
              </button>
            )}
          </div>

          {turns.length === 0 && !chatBusy && (
            <p className="m-0 text-[11.5px] text-term-dim">
              问一个问题，三个脑会依次发言；后发言者能看到前面的内容。
            </p>
          )}

          <div className="flex flex-col gap-2.5">
            {turns.map((turn, index) => {
              const isUser = turn.persona === null;
              const meta = turn.persona ? PERSONA_META[turn.persona] : null;

              return (
                <div
                  key={`${index}-${turn.persona ?? 'user'}`}
                  className={`max-w-[92%] border-l-2 px-2.5 py-1.5 ${
                    isUser
                      ? 'self-end border-term-fg bg-term-grid/30'
                      : 'self-start border-term-cyan/60 bg-black/20'
                  }`}
                >
                  <div className="text-[9px] uppercase tracking-[0.2em] text-term-dim">
                    {isUser ? 'USER' : `${meta?.unit} · ${meta?.label}`}
                  </div>
                  <div className="whitespace-pre-wrap text-[12px] leading-relaxed text-term-fg">
                    {turn.content}
                  </div>
                </div>
              );
            })}

            {activePersona && (
              <div className="max-w-[92%] self-start border-l-2 border-term-amber/60 bg-black/20 px-2.5 py-1.5">
                <div className="text-[9px] uppercase tracking-[0.2em] text-term-amber">
                  {PERSONA_META[activePersona].unit} ·{' '}
                  {PERSONA_META[activePersona].label}
                </div>
                <div className="whitespace-pre-wrap text-[12px] leading-relaxed text-term-fg">
                  {streaming[activePersona]}
                  <span className="animate-[chat-blink_1s_steps(1)_infinite] text-term-amber">
                    ▍
                  </span>
                </div>
              </div>
            )}

            {PERSONAS.filter((persona) => personaErrors[persona]).map(
              (persona) => (
                <div
                  key={`error-${persona}`}
                  className="max-w-[92%] self-start border border-term-red px-2.5 py-1.5 text-[11px] text-term-red"
                >
                  {PERSONA_META[persona].label}: {personaErrors[persona]}
                </div>
              ),
            )}

            <div ref={endRef} />
          </div>
        </div>

        <div className="relative z-10 border-t border-term-grid px-4 py-2.5">
          {finalizeError && <ErrorBlock message={finalizeError} />}
          {!contextText && (
            <p className="mb-2 text-[11px] text-term-amber">
              摘要为空，无法群聊。请先加载全文。
            </p>
          )}

          <div className="flex items-end gap-2">
            <textarea
              className="min-h-[44px] flex-1 resize-none border border-term-grid bg-black/30 px-2.5 py-2 text-[12px] text-term-fg outline-none placeholder:text-term-dim focus:border-term-cyan disabled:opacity-40"
              rows={2}
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (
                  event.key === 'Enter' &&
                  !event.shiftKey &&
                  !event.nativeEvent.isComposing
                ) {
                  event.preventDefault();
                  send();
                }
              }}
              placeholder={
                contextText
                  ? '问评审团…（SHIFT+ENTER 换行）'
                  : '没有可用的论文上下文'
              }
              disabled={!contextText || chatBusy || reviewBusy}
            />
            <div className="flex flex-col gap-1.5">
              <button
                className="border border-term-cyan px-3 py-1.5 text-[10px] tracking-[0.16em] text-term-cyan hover:bg-term-cyan hover:text-term-bg disabled:opacity-40"
                onClick={send}
                disabled={
                  !contextText ||
                  !draft.trim() ||
                  chatBusy ||
                  reviewBusy
                }
              >
                发送
              </button>
              <button
                className="border border-term-amber px-3 py-1.5 text-[10px] tracking-[0.16em] text-term-amber hover:bg-term-amber hover:text-term-bg disabled:opacity-40"
                onClick={() => void finalize()}
                disabled={
                  finalizing ||
                  chatBusy ||
                  turns.length === 0 ||
                  !contextText
                }
              >
                {finalizing ? '表决中…' : '形成结论'}
              </button>
            </div>
          </div>

          <div className="mt-1.5 flex items-center justify-between text-[9px] tracking-[0.14em] text-term-dim">
            <span>
              {chatBusy && activePersona
                ? `${PERSONA_META[activePersona].label} 发言中…`
                : 'ENTER 发送 · SHIFT+ENTER 换行 · ESC 关闭'}
            </span>
            <span>
              MODEL {review?.model || '—'} · PROMPT{' '}
              {review?.prompt_version || '—'}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
