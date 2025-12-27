import { useState } from "react";
import type { Reminder } from "../types";

interface CompletedSectionProps {
  reminders: Reminder[];
  onDelete: (id: number) => void;
}

export function CompletedSection({ reminders, onDelete }: CompletedSectionProps) {
  const [expanded, setExpanded] = useState(false);

  if (reminders.length === 0) return null;

  return (
    <div className="mt-4 pt-4 border-t border-dark-700">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 text-gray-500 hover:text-gray-400 text-xs"
      >
        <span>{expanded ? "▼" : "▶"}</span>
        <span>Completed ({reminders.length})</span>
      </button>

      {expanded && (
        <div className="mt-2 space-y-1">
          {reminders.map((reminder) => (
            <div
              key={reminder.id}
              className="px-3 py-1.5 flex items-center gap-2 text-gray-500 group"
            >
              <span className="text-accent-green text-xs">✓</span>
              <p className="flex-1 text-xs line-through truncate">{reminder.message}</p>
              <button
                onClick={() => onDelete(reminder.id)}
                className="opacity-0 group-hover:opacity-100 text-gray-600 hover:text-accent-red text-xs transition-opacity"
              >
                ✕
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
