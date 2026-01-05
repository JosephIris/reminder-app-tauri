import { useState, useEffect, KeyboardEvent, RefObject } from "react";
import { parseNaturalTime } from "../utils/time";

interface ReminderInputProps {
  onAdd: (message: string, dueTime: Date, recurrence?: string) => Promise<void>;
  syncing: boolean;
  inputRef?: RefObject<HTMLInputElement | null>;
}

const placeholderExamples = [
  "call mom in 2 hours",
  "meeting at 3pm",
  "submit report tomorrow",
  "dentist appointment friday 10am",
  "pick up groceries in 30 min",
];

export function ReminderInput({ onAdd, syncing, inputRef }: ReminderInputProps) {
  const [text, setText] = useState("");
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
        }, 50 + Math.random() * 30); // Slight randomness for natural feel
        return () => clearTimeout(timeout);
      } else {
        // Finished typing, wait then start erasing
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
        // Finished erasing, move to next example
        setPlaceholderIndex((prev) => (prev + 1) % placeholderExamples.length);
        setIsTyping(true);
      }
    }
  }, [displayedPlaceholder, isTyping, placeholderIndex]);

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
    <div className="space-y-2">
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
    </div>
  );
}
