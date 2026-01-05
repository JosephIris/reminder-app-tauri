import { useState, useEffect, useRef, useCallback } from "react";
import type { Reminder } from "../types";
import { formatTimeWithAbsolute, getUrgencyLevel } from "../utils/time";
import "./ReminderItem.css";

interface ReminderItemProps {
  reminder: Reminder;
  isFocused?: boolean;
  isLeaving?: boolean;
  isDragging?: boolean;
  isDragOver?: boolean;
  onComplete: (id: number) => void;
  onDelete: (id: number) => void;
  onSnooze: (id: number, minutes: number) => void;
  onEdit: (reminder: Reminder) => void;
  onFocus?: (id: number | null) => void;
  onMouseDown?: (e: React.MouseEvent) => void;
}

const urgencyColors = {
  overdue: "text-red-400 bg-red-500/20",
  urgent: "text-orange-400 bg-orange-500/20",
  soon: "text-yellow-400 bg-yellow-500/20",
  normal: "text-gray-400 bg-dark-600",
};

export function ReminderItem({
  reminder,
  isFocused,
  isLeaving,
  isDragging,
  isDragOver,
  onComplete,
  onDelete,
  onSnooze,
  onEdit,
  onFocus,
  onMouseDown,
}: ReminderItemProps) {
  const [isHovered, setIsHovered] = useState(false);
  const [isCompleting, setIsCompleting] = useState(false);
  const itemRef = useRef<HTMLDivElement>(null);
  const dueTime = new Date(reminder.due_time);
  const urgency = getUrgencyLevel(dueTime);
  const { relative, absolute } = formatTimeWithAbsolute(dueTime);

  const handleComplete = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setIsCompleting(true);
    // Delay the actual complete to show animation
    setTimeout(() => {
      onComplete(reminder.id);
    }, 300);
  }, [onComplete, reminder.id]);

  // Auto-scroll into view when focused
  useEffect(() => {
    if (isFocused && itemRef.current) {
      itemRef.current.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }, [isFocused]);

  const handleClick = () => {
    if (onFocus) {
      onFocus(isFocused ? null : reminder.id);
    }
  };

  return (
    <div
      ref={itemRef}
      className={`reminder-item-wrapper ${isFocused ? "focused" : ""} ${isLeaving ? "leaving" : ""} ${urgency === "urgent" ? "pulse-urgent" : ""} ${isDragging ? "dragging" : ""} ${isDragOver ? "drag-over" : ""}`}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onDoubleClick={() => onEdit(reminder)}
      onClick={handleClick}
      onMouseDown={onMouseDown}
    >
      <div className="reminder-item-inner">
        {/* Message */}
        <p className="flex-1 text-white text-sm truncate">{reminder.message}</p>

        {/* Time badge with urgency color */}
        <div
          className={`flex items-center gap-1.5 px-2 py-0.5 rounded-md text-xs whitespace-nowrap ${urgencyColors[urgency]}`}
          title={`Due at ${absolute}`}
        >
          <span className="font-medium">{relative}</span>
          <span className="opacity-60 text-[10px]">({absolute})</span>
        </div>

        {/* Actions - show on hover or focus */}
        <div className={`flex items-center gap-1 transition-opacity duration-200 ${isHovered || isFocused ? "opacity-100" : "opacity-0"}`}>
          <button
            onClick={(e) => { e.stopPropagation(); onSnooze(reminder.id, 15); }}
            className="px-1.5 py-0.5 text-[10px] text-gray-400 hover:text-white hover:bg-dark-600 rounded transition-colors active:scale-90"
            title="Snooze 15min"
          >
            15
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); onSnooze(reminder.id, 60); }}
            className="px-1.5 py-0.5 text-[10px] text-gray-400 hover:text-white hover:bg-dark-600 rounded transition-colors active:scale-90"
            title="Snooze 1hr"
          >
            60
          </button>
          <button
            onClick={handleComplete}
            disabled={isCompleting}
            className={`w-6 h-6 flex items-center justify-center text-gray-400 hover:text-accent-green hover:bg-dark-600 rounded transition-all ${isCompleting ? "text-accent-green scale-110" : "active:scale-90"}`}
            title="Complete"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="3"
              strokeLinecap="round"
              strokeLinejoin="round"
              className={isCompleting ? "check-icon animate" : ""}
            >
              <polyline points="20 6 9 17 4 12" />
            </svg>
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); onDelete(reminder.id); }}
            className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-accent-red hover:bg-dark-600 rounded transition-colors active:scale-90"
            title="Delete"
          >
            ✕
          </button>
        </div>
      </div>
    </div>
  );
}
