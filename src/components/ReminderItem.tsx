import { useState, useEffect, useRef } from "react";
import type { Reminder } from "../types";
import { formatTimeWithAbsolute, getUrgencyLevel } from "../utils/time";
import "./ReminderItem.css";

interface ReminderItemProps {
  reminder: Reminder;
  index: number;
  isFocused?: boolean;
  isLeaving?: boolean;
  isDragging?: boolean;
  isDragOver?: boolean;
  onComplete: (id: number) => void;
  onDelete: (id: number) => void;
  onSnooze: (id: number, minutes: number) => void;
  onEdit: (reminder: Reminder) => void;
  onFocus?: (id: number | null) => void;
  onDragStart?: (e: React.DragEvent, index: number) => void;
  onDragOver?: (e: React.DragEvent, index: number) => void;
  onDrop?: (e: React.DragEvent, index: number) => void;
  onDragEnd?: () => void;
}

const urgencyColors = {
  overdue: "text-red-400 bg-red-500/20",
  urgent: "text-orange-400 bg-orange-500/20",
  soon: "text-yellow-400 bg-yellow-500/20",
  normal: "text-gray-400 bg-dark-600",
};

export function ReminderItem({
  reminder,
  index,
  isFocused,
  isLeaving,
  isDragging,
  isDragOver,
  onComplete,
  onDelete,
  onSnooze,
  onEdit,
  onFocus,
  onDragStart,
  onDragOver,
  onDrop,
  onDragEnd,
}: ReminderItemProps) {
  const [isHovered, setIsHovered] = useState(false);
  const itemRef = useRef<HTMLDivElement>(null);
  const dueTime = new Date(reminder.due_time);
  const urgency = getUrgencyLevel(dueTime);
  const { relative, absolute } = formatTimeWithAbsolute(dueTime);

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
      draggable
      className={`reminder-item-wrapper ${isFocused ? "focused" : ""} ${isLeaving ? "leaving" : ""} ${urgency === "urgent" ? "pulse-urgent" : ""} ${isDragging ? "dragging" : ""} ${isDragOver ? "drag-over" : ""}`}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onDoubleClick={() => onEdit(reminder)}
      onClick={handleClick}
      onDragStart={(e) => onDragStart?.(e, index)}
      onDragOver={(e) => {
        e.preventDefault();
        onDragOver?.(e, index);
      }}
      onDrop={(e) => onDrop?.(e, index)}
      onDragEnd={onDragEnd}
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
            className="px-1.5 py-0.5 text-[10px] text-gray-400 hover:text-white hover:bg-dark-600 rounded transition-colors"
            title="Snooze 15min"
          >
            15
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); onSnooze(reminder.id, 60); }}
            className="px-1.5 py-0.5 text-[10px] text-gray-400 hover:text-white hover:bg-dark-600 rounded transition-colors"
            title="Snooze 1hr"
          >
            60
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); onComplete(reminder.id); }}
            className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-accent-green hover:bg-dark-600 rounded transition-colors"
            title="Complete"
          >
            ✓
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); onDelete(reminder.id); }}
            className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-accent-red hover:bg-dark-600 rounded transition-colors"
            title="Delete"
          >
            ✕
          </button>
        </div>
      </div>
    </div>
  );
}
