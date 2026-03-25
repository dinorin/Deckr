import { Bot, Check, Copy, ExternalLink, Loader2 } from 'lucide-react';
import { useEffect, useRef } from 'react';
import ReactMarkdown from 'react-markdown';
import type { AgentLogEntry } from '../services/llm';
import type { AgentStatus, Message } from '../types';

const AGENT_LABELS: Record<string, string> = {
  orchestrator: 'Orchestrator',
  search: 'Search',
  content: 'Content',
  design: 'Design',
  slides: 'Renderer',
  edit: 'Editor',
  html: 'Renderer',
};

interface MessageListProps {
  messages: Message[];
  isLoading: boolean;
  agentStatus: AgentStatus | null;
  agentLog: AgentLogEntry[];
  onCopy: (text: string) => void;
}

function AgentPipeline({ log, currentMessage }: { log: AgentLogEntry[]; currentMessage: string }) {
  // Build display list: dedupe by agent, show latest status per agent
  const agentMap = new Map<string, AgentLogEntry>();
  for (const entry of log) {
    agentMap.set(entry.agent, entry);
  }
  const entries = Array.from(agentMap.values());

  return (
    <div className="flex gap-3 items-start">
      <div className="w-6 h-6 bg-input border border-line rounded-md flex items-center justify-center shrink-0 mt-0.5">
        <Bot size={12} strokeWidth={2} className="text-fg-3" />
      </div>
      <div className="flex-1 min-w-0 space-y-2 pt-0.5">
        {entries.length > 0 ? (
          <div className="space-y-2">
            {entries.map((entry) => (
              <div key={entry.agent} className="space-y-1.5">
                <div className="flex items-center gap-2.5">
                  {entry.status === 'done' ? (
                    <div className="w-4 h-4 rounded-full bg-cta/15 flex items-center justify-center shrink-0">
                      <Check size={9} className="text-cta" strokeWidth={3} />
                    </div>
                  ) : (
                    <Loader2 size={14} className="text-fg-4 animate-spin shrink-0" />
                  )}
                  <span className={`text-[13px] leading-none ${entry.status === 'done' ? 'text-fg-4' : 'text-fg-3'}`}>
                    <span className="font-medium">{AGENT_LABELS[entry.agent] ?? entry.agent}</span>
                    {' — '}
                    {entry.message}
                  </span>
                </div>

                {entry.agent === 'search' && entry.status === 'done' && entry.images && entry.images.length > 0 && (
                  <div className="ml-6 flex gap-1.5 overflow-x-auto pb-0.5">
                    {entry.images.map((url, i) => (
                      <img
                        key={i}
                        src={url}
                        alt=""
                        className="h-12 w-20 object-cover rounded shrink-0 opacity-75 hover:opacity-100 transition-opacity"
                        onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }}
                      />
                    ))}
                  </div>
                )}

                {entry.agent === 'search' && entry.status === 'done' && entry.links && entry.links.length > 0 && (
                  <div className="ml-6 flex flex-wrap gap-1.5">
                    {entry.links.map((link, i) => (
                      <a
                        key={i}
                        href={link.url}
                        target="_blank"
                        rel="noreferrer"
                        className="flex items-center gap-1 px-2 py-0.5 rounded bg-input border border-line text-[11px] text-fg-4 hover:text-fg-2 hover:border-fg-4 transition-colors max-w-[180px]"
                        title={link.title}
                      >
                        <ExternalLink size={9} className="shrink-0" />
                        <span className="truncate">{link.title}</span>
                      </a>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        ) : (
          <div className="flex items-center gap-2">
            <Loader2 size={14} className="text-fg-4 animate-spin shrink-0" />
            <span className="text-[13px] text-fg-3">{currentMessage}</span>
          </div>
        )}
      </div>
    </div>
  );
}

export function MessageList({ messages, isLoading, agentStatus, agentLog, onCopy }: MessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  // Scroll only when a new message is ADDED or loading/agent state changes —
  // NOT when an existing message's content updates (typing animation every 18ms).
  const messageCount = messages.length;
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messageCount, isLoading, agentLog.length]);

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="max-w-[760px] mx-auto px-6 py-8 space-y-8">
        {messages.map(msg => (
          <div key={msg.id}>
            {msg.role === 'user' ? (
              <div className="flex justify-end">
                <div className="max-w-[75%] bg-tint border border-tint-bd rounded-2xl px-4 py-2.5 text-[15px] text-fg leading-relaxed">
                  <p className="whitespace-pre-wrap">{msg.content}</p>
                </div>
              </div>
            ) : (
              <div className="flex gap-3">
                <div className="w-6 h-6 bg-tint border border-tint-bd rounded-md flex items-center justify-center shrink-0 mt-0.5">
                  <Bot size={12} strokeWidth={2} className="text-cta" />
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-[15px] leading-relaxed prose prose-sm max-w-none
                    prose-p:text-[color:var(--fg-2)] prose-p:my-1.5 prose-p:leading-relaxed
                    prose-headings:text-[color:var(--fg)] prose-headings:font-semibold prose-headings:mt-4 prose-headings:mb-2
                    prose-strong:text-[color:var(--fg)] prose-strong:font-semibold
                    prose-code:text-[color:var(--fg-3)] prose-code:bg-[color:var(--input)] prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded prose-code:text-[13px]
                    prose-pre:bg-[color:var(--card)] prose-pre:border prose-pre:border-[color:var(--line)] prose-pre:rounded-lg
                    prose-li:text-[color:var(--fg-2)] prose-ul:my-1.5 prose-ol:my-1.5
                    prose-a:text-[color:var(--fg-3)] prose-a:no-underline hover:prose-a:underline
                    prose-hr:border-[color:var(--line)]">
                    <ReactMarkdown>{msg.content || '…'}</ReactMarkdown>
                  </div>
                  {msg.content && (
                    <button
                      onClick={() => onCopy(msg.content)}
                      className="mt-2 flex items-center gap-1.5 px-2 py-1 -ml-2 rounded-md text-[13px] text-fg-4 hover:text-fg-2 hover:bg-input transition-colors"
                    >
                      <Copy size={11} /> Copy
                    </button>
                  )}
                </div>
              </div>
            )}
          </div>
        ))}

        {isLoading && (
          <AgentPipeline
            log={agentLog}
            currentMessage={agentStatus?.message ?? 'Starting...'}
          />
        )}

        <div ref={bottomRef} />
      </div>
    </div>
  );
}
