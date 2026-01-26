import { useState, useEffect, useRef } from "react";
import type { Reminder, UrgencyType } from "../types";

interface EditDialogProps {
  reminder: Reminder;
  onSave: (id: number, message: string, urgency: UrgencyType) => Promise<void>;
  onClose: () => void;
}

const urgencyOptions: { value: UrgencyType; label: string; color: string }[] = [
  { value: "now", label: "Now", color: "text-red-400" },
  { value: "today", label: "Today", color: "text-orange-400" },
  { value: "soon", label: "Soon", color: "text-yellow-400" },
  { value: "whenever", label: "Whenever", color: "text-gray-400" },
];

export function EditDialog({ reminder, onSave, onClose }: EditDialogProps) {
  const [message, setMessage] = useState(reminder.message);
  const [urgency, setUrgency] = useState<UrgencyType>(reminder.urgency);
  const [saving, setSaving] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // Focus input on mount
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [onClose]);

  const handleSave = async () => {
    if (!message.trim() || saving) return;

    setSaving(true);
    try {
      await onSave(reminder.id, message.trim(), urgency);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSave();
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 animate-fade-in">
      <div
        ref={dialogRef}
        className="bg-dark-800 rounded-xl p-6 w-full max-w-md shadow-2xl animate-slide-up"
      >
        <h2 className="text-lg font-semibold text-white mb-4">Edit Task</h2>

        <div className="space-y-4">
          {/* Message */}
          <div>
            <label className="block text-sm text-gray-400 mb-1">Message</label>
            <input
              ref={inputRef}
              type="text"
              value={message}
              onChange={(e) => setMessage(e.target.value)}
              onKeyDown={handleKeyDown}
              className="w-full bg-dark-700 border border-dark-500 rounded-lg px-3 py-2
                         text-white focus:outline-none focus:border-accent-blue"
            />
          </div>

          {/* Urgency */}
          <div>
            <label className="block text-sm text-gray-400 mb-2">Urgency</label>
            <div className="flex gap-2">
              {urgencyOptions.map((option) => (
                <button
                  key={option.value}
                  onClick={() => setUrgency(option.value)}
                  className={`flex-1 px-3 py-2 text-sm rounded-lg border transition-all ${
                    urgency === option.value
                      ? `${option.color} border-current bg-dark-600`
                      : "text-gray-500 border-dark-500 hover:text-gray-300 hover:border-dark-400"
                  }`}
                >
                  {option.label}
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* Actions */}
        <div className="flex justify-end gap-3 mt-6">
          <button
            onClick={onClose}
            className="px-4 py-2 text-gray-400 hover:text-white transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={saving || !message.trim()}
            className="px-4 py-2 bg-accent-blue hover:bg-blue-600 disabled:bg-dark-600
                       text-white rounded-lg transition-colors"
          >
            {saving ? "Saving..." : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
