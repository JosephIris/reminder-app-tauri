import { useState, useEffect, useRef, useCallback } from "react";
import type { Reminder, UrgencyType, ListType } from "../types";
import "./ReminderItem.css";

interface ReminderItemProps {
  reminder: Reminder;
  isFocused?: boolean;
  isLeaving?: boolean;
  isDragging?: boolean;
  isDragOver?: boolean;
  onComplete: (id: number) => void;
  onDelete: (id: number) => void;
  onEdit: (reminder: Reminder) => void;
  onMove?: (id: number, toList: ListType) => void;
  onSetUrgency?: (id: number, urgency: UrgencyType) => void;
  onFocus?: (id: number | null) => void;
  onMouseDown?: (e: React.MouseEvent) => void;
}

const urgencyConfig: Record<UrgencyType, { label: string; color: string; bgColor: string }> = {
  now: { label: "NOW", color: "text-red-400", bgColor: "bg-red-500/20" },
  today: { label: "Today", color: "text-orange-400", bgColor: "bg-orange-500/20" },
  soon: { label: "Soon", color: "text-yellow-400", bgColor: "bg-yellow-500/20" },
  whenever: { label: "Whenever", color: "text-gray-400", bgColor: "bg-dark-600" },
};

export function ReminderItem({
  reminder,
  isFocused,
  isLeaving,
  isDragging,
  isDragOver,
  onComplete,
  onDelete,
  onEdit,
  onMove,
  onSetUrgency,
  onFocus,
  onMouseDown,
}: ReminderItemProps) {
  const [isHovered, setIsHovered] = useState(false);
  const [isCompleting, setIsCompleting] = useState(false);
  const [showUrgencyMenu, setShowUrgencyMenu] = useState(false);
  const itemRef = useRef<HTMLDivElement>(null);
  const urgencyMenuRef = useRef<HTMLDivElement>(null);

  const urgency = urgencyConfig[reminder.urgency] || urgencyConfig.whenever;

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

  // Close urgency menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (urgencyMenuRef.current && !urgencyMenuRef.current.contains(e.target as Node)) {
        setShowUrgencyMenu(false);
      }
    };
    if (showUrgencyMenu) {
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [showUrgencyMenu]);

  const handleClick = () => {
    if (onFocus) {
      onFocus(isFocused ? null : reminder.id);
    }
  };

  const handleUrgencyClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    setShowUrgencyMenu(!showUrgencyMenu);
  };

  const handleUrgencySelect = (newUrgency: UrgencyType) => {
    if (onSetUrgency && newUrgency !== reminder.urgency) {
      onSetUrgency(reminder.id, newUrgency);
    }
    setShowUrgencyMenu(false);
  };

  const handleMoveClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (onMove) {
      const targetList: ListType = reminder.list_type === "actual" ? "backlog" : "actual";
      onMove(reminder.id, targetList);
    }
  };

  return (
    <div
      ref={itemRef}
      className={`reminder-item-wrapper ${isFocused ? "focused" : ""} ${isLeaving ? "leaving" : ""} ${reminder.urgency === "now" ? "pulse-urgent" : ""} ${isDragging ? "dragging" : ""} ${isDragOver ? "drag-over" : ""}`}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onDoubleClick={() => onEdit(reminder)}
      onClick={handleClick}
      onMouseDown={onMouseDown}
    >
      <div className="reminder-item-inner">
        {/* Message */}
        <p className="flex-1 text-white text-sm truncate">{reminder.message}</p>

        {/* Urgency badge - clickable to change */}
        <div className="relative" ref={urgencyMenuRef}>
          <button
            onClick={handleUrgencyClick}
            className={`flex items-center gap-1 px-2 py-0.5 rounded-md text-xs whitespace-nowrap transition-colors hover:opacity-80 ${urgency.color} ${urgency.bgColor}`}
            title="Click to change urgency"
          >
            <span className="font-medium">{urgency.label}</span>
          </button>

          {/* Urgency dropdown menu - opens upward to avoid clipping */}
          {showUrgencyMenu && (
            <div className="absolute right-0 bottom-full mb-1 bg-dark-700 border border-dark-500 rounded-lg shadow-xl z-50 py-1 min-w-[100px]">
              {(Object.keys(urgencyConfig) as UrgencyType[]).map((key) => (
                <button
                  key={key}
                  onClick={(e) => { e.stopPropagation(); handleUrgencySelect(key); }}
                  className={`w-full px-3 py-1.5 text-left text-xs hover:bg-dark-600 transition-colors ${urgencyConfig[key].color} ${reminder.urgency === key ? "bg-dark-600" : ""}`}
                >
                  {urgencyConfig[key].label}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Actions - show on hover or focus */}
        <div className={`flex items-center gap-1 transition-opacity duration-200 ${isHovered || isFocused ? "opacity-100" : "opacity-0"}`}>
          {/* Move to actual/backlog button */}
          {onMove && (
            <button
              onClick={handleMoveClick}
              className="px-1.5 py-0.5 text-[10px] text-gray-400 hover:text-white hover:bg-dark-600 rounded transition-colors active:scale-90"
              title={reminder.list_type === "actual" ? "Move to backlog" : "Move to actual"}
            >
              {reminder.list_type === "actual" ? "→BL" : "→AC"}
            </button>
          )}
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
