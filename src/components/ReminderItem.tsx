import { useState } from "react";
import type { Reminder } from "../types";
import { formatRelativeTime } from "../utils/time";

interface ReminderItemProps {
  reminder: Reminder;
  onComplete: (id: number) => void;
  onDelete: (id: number) => void;
  onSnooze: (id: number, minutes: number) => void;
  onEdit: (reminder: Reminder) => void;
}

export function ReminderItem({ reminder, onComplete, onDelete, onSnooze, onEdit }: ReminderItemProps) {
  const [isHovered, setIsHovered] = useState(false);
  const dueTime = new Date(reminder.due_time);
  const isPastDue = dueTime <= new Date();

  return (
    <div
      className="bg-dark-700 rounded-lg px-3 py-2 flex items-center gap-3 group"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onDoubleClick={() => onEdit(reminder)}
    >
      {/* Message */}
      <p className="flex-1 text-white text-sm truncate">{reminder.message}</p>

      {/* Time */}
      <span className={`text-xs whitespace-nowrap ${isPastDue ? "text-accent-red" : "text-gray-500"}`}>
        {formatRelativeTime(dueTime)}
      </span>

      {/* Actions - show on hover */}
      <div className={`flex items-center gap-1 transition-opacity ${isHovered ? "opacity-100" : "opacity-0"}`}>
        <button
          onClick={() => onSnooze(reminder.id, 15)}
          className="px-1.5 py-0.5 text-[10px] text-gray-400 hover:text-white hover:bg-dark-600 rounded transition-colors"
          title="Snooze 15min"
        >
          15
        </button>
        <button
          onClick={() => onSnooze(reminder.id, 60)}
          className="px-1.5 py-0.5 text-[10px] text-gray-400 hover:text-white hover:bg-dark-600 rounded transition-colors"
          title="Snooze 1hr"
        >
          60
        </button>
        <button
          onClick={() => onComplete(reminder.id)}
          className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-accent-green hover:bg-dark-600 rounded transition-colors"
          title="Complete"
        >
          ✓
        </button>
        <button
          onClick={() => onDelete(reminder.id)}
          className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-accent-red hover:bg-dark-600 rounded transition-colors"
          title="Delete"
        >
          ✕
        </button>
      </div>
    </div>
  );
}
