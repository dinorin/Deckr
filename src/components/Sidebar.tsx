import { MessageSquarePlus, Trash2 } from 'lucide-react';
import { cn } from '../lib/utils';
import type { SessionSummary } from '../types';

interface SidebarProps {
  sessions: SessionSummary[];
  currentId: string;
  onNewChat: () => void;
  onSwitchSession: (id: string) => void;
  onDeleteSession: (id: string) => void;
}

function getGroup(updatedAt: number): string {
  const now = Date.now();
  const diff = now - updatedAt;
  const day = 86400000;
  if (diff < day) return 'Today';
  if (diff < 2 * day) return 'Yesterday';
  if (diff < 7 * day) return 'This week';
  if (diff < 14 * day) return 'Last week';
  if (diff < 30 * day) return 'This month';
  return 'Older';
}

const GROUP_ORDER = ['Today', 'Yesterday', 'This week', 'Last week', 'This month', 'Older'];

export function Sidebar({ sessions, currentId, onNewChat, onSwitchSession, onDeleteSession }: SidebarProps) {
  const grouped = GROUP_ORDER.reduce<Record<string, SessionSummary[]>>((acc, g) => {
    const items = sessions.filter(s => getGroup(s.updatedAt) === g);
    if (items.length) acc[g] = items;
    return acc;
  }, {});

  return (
    <div className="flex flex-col h-full bg-bg border-r border-line w-full">
      {/* New chat button */}
      <div className="p-2.5 border-b border-line">
        <button
          onClick={onNewChat}
          className="w-full flex items-center gap-2 px-3 py-2 rounded-lg bg-input hover:bg-line text-fg text-[14px] font-medium transition-colors"
        >
          <MessageSquarePlus size={14} className="text-fg-3" />
          New Presentation
        </button>
      </div>

      {/* Sessions */}
      <div className="flex-1 overflow-y-auto py-2 px-2">
        {sessions.length === 0 ? (
          <p className="text-[14px] text-fg-3 text-center py-8">No presentations yet</p>
        ) : (
          Object.entries(grouped).map(([group, items]) => (
            <div key={group} className="mb-3">
              <p className="text-[12px] text-fg-4 font-medium px-2 mb-1 uppercase tracking-wider">{group}</p>
              {items.map(s => (
                <div
                  key={s.id}
                  onClick={() => onSwitchSession(s.id)}
                  className={cn(
                    'group flex items-center gap-2 px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors mb-0.5',
                    s.id === currentId
                      ? 'bg-tint text-fg'
                      : 'text-fg-3 hover:bg-card hover:text-fg-2'
                  )}
                >
                  <span className="flex-1 text-[14px] truncate leading-tight">{s.title}</span>
                  <button
                    onClick={e => { e.stopPropagation(); onDeleteSession(s.id); }}
                    className="opacity-0 group-hover:opacity-100 p-1.5 rounded-md text-fg-4 hover:text-red-400 transition-all shrink-0"
                  >
                    <Trash2 size={12} />
                  </button>
                </div>
              ))}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
