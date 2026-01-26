import { useState, useEffect, KeyboardEvent, RefObject } from "react";
import type { UrgencyType, ListType } from "../types";

interface ReminderInputProps {
  onAdd: (message: string, urgency: UrgencyType, listType: ListType) => void;
  syncing: boolean;
  inputRef?: RefObject<HTMLInputElement | null>;
}

const placeholderExamples = [
  "Fix login bug",
  "Review pull request",
  "Call the dentist",
  "Write documentation",
  "Prepare presentation",
];

const urgencyOptions: { value: UrgencyType; label: string; color: string }[] = [
  { value: "now", label: "Now", color: "text-red-400 border-red-500/50" },
  { value: "today", label: "Today", color: "text-orange-400 border-orange-500/50" },
  { value: "soon", label: "Soon", color: "text-yellow-400 border-yellow-500/50" },
  { value: "whenever", label: "Whenever", color: "text-gray-400 border-gray-500/50" },
];

const listOptions: { value: ListType; label: string }[] = [
  { value: "actual", label: "Actual" },
  { value: "backlog", label: "Backlog" },
];

export function ReminderInput({ onAdd, syncing, inputRef }: ReminderInputProps) {
  const [text, setText] = useState("");
  const [urgency, setUrgency] = useState<UrgencyType>("today");
  const [listType, setListType] = useState<ListType>("actual");
  const [placeholderIndex, setPlaceholderIndex] = useState(0);
  const [displayedPlaceholder, setDisplayedPlaceholder] = useState("");
  const [isTyping, setIsTyping] = useState(true);

  useEffect(() => {
    // Auto-focus on mount
    inputRef?.current?.focus();
  }, []);

  // Typewriter effect for placeholder
  useEffect(() => {
    const currentExample = placeholderExamples[placeholderIndex];

    if (isTyping) {
      if (displayedPlaceholder.length < currentExample.length) {
        const timeout = setTimeout(() => {
          setDisplayedPlaceholder(currentExample.slice(0, displayedPlaceholder.length + 1));
        }, 50 + Math.random() * 30);
        return () => clearTimeout(timeout);
      } else {
        const timeout = setTimeout(() => setIsTyping(false), 2000);
        return () => clearTimeout(timeout);
      }
    } else {
      if (displayedPlaceholder.length > 0) {
        const timeout = setTimeout(() => {
          setDisplayedPlaceholder(displayedPlaceholder.slice(0, -1));
        }, 30);
        return () => clearTimeout(timeout);
      } else {
        setPlaceholderIndex((prev) => (prev + 1) % placeholderExamples.length);
        setIsTyping(true);
      }
    }
  }, [displayedPlaceholder, isTyping, placeholderIndex]);

  const handleSubmit = () => {
    if (!text.trim() || syncing) return;
    onAdd(text.trim(), urgency, listType);
    setText("");
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      handleSubmit();
    }
  };

  return (
    <div className="space-y-2">
      {/* Main input row */}
      <div className="flex gap-2">
        <input
          ref={inputRef as RefObject<HTMLInputElement>}
          type="text"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={displayedPlaceholder || "..."}
          className="flex-1 bg-dark-700 border border-dark-500 rounded-lg px-4 py-3
                     text-white placeholder-gray-500 focus:outline-none focus:border-accent-blue
                     focus:ring-2 focus:ring-accent-blue/30 input-glow"
          disabled={syncing}
        />
        <button
          onClick={handleSubmit}
          disabled={syncing || !text.trim()}
          className="bg-accent-blue hover:bg-blue-600 disabled:bg-dark-600 disabled:cursor-not-allowed
                     text-white font-medium px-6 py-3 rounded-lg transition-all
                     flex items-center justify-center min-w-[56px]
                     hover:scale-105 active:scale-95"
        >
          {syncing ? (
            <span className="animate-pulse-soft">...</span>
          ) : (
            <span className="text-xl">+</span>
          )}
        </button>
      </div>

      {/* Urgency and List selectors row */}
      <div className="flex gap-4 items-center">
        {/* Urgency selector */}
        <div className="flex items-center gap-1">
          <span className="text-xs text-gray-500 mr-1">Urgency:</span>
          {urgencyOptions.map((option) => (
            <button
              key={option.value}
              onClick={() => setUrgency(option.value)}
              className={`px-2 py-1 text-xs rounded border transition-all ${
                urgency === option.value
                  ? `${option.color} bg-dark-600`
                  : "text-gray-500 border-transparent hover:text-gray-300"
              }`}
            >
              {option.label}
            </button>
          ))}
        </div>

        {/* List type selector */}
        <div className="flex items-center gap-1">
          <span className="text-xs text-gray-500 mr-1">List:</span>
          {listOptions.map((option) => (
            <button
              key={option.value}
              onClick={() => setListType(option.value)}
              className={`px-2 py-1 text-xs rounded border transition-all ${
                listType === option.value
                  ? "text-accent-blue border-accent-blue/50 bg-dark-600"
                  : "text-gray-500 border-transparent hover:text-gray-300"
              }`}
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
