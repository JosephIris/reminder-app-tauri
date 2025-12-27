import { useState, useEffect, KeyboardEvent, RefObject } from "react";
import { parseNaturalTime } from "../utils/time";

interface ReminderInputProps {
  onAdd: (message: string, dueTime: Date, recurrence?: string) => Promise<void>;
  syncing: boolean;
  inputRef?: RefObject<HTMLInputElement | null>;
}

export function ReminderInput({ onAdd, syncing, inputRef }: ReminderInputProps) {
  const [text, setText] = useState("");

  useEffect(() => {
    // Auto-focus on mount
    inputRef?.current?.focus();
  }, []);

  const handleSubmit = async () => {
    if (!text.trim() || syncing) return;

    const parsed = parseNaturalTime(text);
    if (parsed) {
      await onAdd(parsed.message, parsed.dueTime);
      setText("");
    }
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      handleSubmit();
    }
  };

  return (
    <div className="flex gap-2">
      <input
        ref={inputRef as RefObject<HTMLInputElement>}
        type="text"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="e.g., call mom in 2 hours"
        className="flex-1 bg-dark-700 border border-dark-500 rounded-lg px-4 py-3
                   text-white placeholder-gray-500 focus:outline-none focus:border-accent-blue
                   focus:ring-1 focus:ring-accent-blue"
        disabled={syncing}
      />
      <button
        onClick={handleSubmit}
        disabled={syncing || !text.trim()}
        className="bg-accent-blue hover:bg-blue-600 disabled:bg-dark-600 disabled:cursor-not-allowed
                   text-white font-medium px-6 py-3 rounded-lg transition-colors
                   flex items-center justify-center min-w-[56px]"
      >
        {syncing ? (
          <span className="animate-pulse-soft">...</span>
        ) : (
          <span className="text-xl">+</span>
        )}
      </button>
    </div>
  );
}
