import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

interface OrganizePromptProps {
  actualCount: number;
  backlogCount: number;
  completedToday: number;
  onOpenTasks: () => void;
}

export function OrganizePrompt({
  actualCount,
  backlogCount,
  completedToday,
  onOpenTasks,
}: OrganizePromptProps) {
  const [visible, setVisible] = useState(false);
  const [snoozedUntil, setSnoozedUntil] = useState<number | null>(null);

  useEffect(() => {
    // Listen for organization prompt events from backend
    const unlisten = listen("organization-prompt", () => {
      const now = Date.now();
      // Don't show if snoozed
      if (snoozedUntil && now < snoozedUntil) {
        return;
      }
      setVisible(true);
    });

    return () => {
      unlisten.then((fn) => fn()).catch(console.error);
    };
  }, [snoozedUntil]);

  const handleDismiss = async () => {
    setVisible(false);
    // Mark as dismissed so backend won't trigger again until next scheduled time
    try {
      await invoke("dismiss_organize_prompt");
    } catch (e) {
      console.error("Failed to dismiss organize prompt:", e);
    }
  };

  const handleSnooze = () => {
    // Snooze for 1 hour
    setSnoozedUntil(Date.now() + 60 * 60 * 1000);
    setVisible(false);
  };

  const handleOpenTasks = () => {
    setVisible(false);
    onOpenTasks();
  };

  if (!visible) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 animate-fade-in">
      <div className="bg-dark-800 rounded-xl p-6 w-full max-w-sm shadow-2xl animate-slide-up">
        {/* Header */}
        <div className="text-center mb-4">
          <div className="text-3xl mb-2">&#128221;</div>
          <h2 className="text-lg font-semibold text-white">Time to Organize!</h2>
          <p className="text-sm text-gray-400 mt-1">
            Review your actual and backlog tasks
          </p>
        </div>

        {/* Stats */}
        <div className="grid grid-cols-3 gap-3 mb-6">
          <div className="bg-dark-700 rounded-lg p-3 text-center">
            <div className="text-xl font-bold text-accent-blue">{actualCount}</div>
            <div className="text-xs text-gray-500">Actual</div>
          </div>
          <div className="bg-dark-700 rounded-lg p-3 text-center">
            <div className="text-xl font-bold text-gray-400">{backlogCount}</div>
            <div className="text-xs text-gray-500">Backlog</div>
          </div>
          <div className="bg-dark-700 rounded-lg p-3 text-center">
            <div className="text-xl font-bold text-accent-green">{completedToday}</div>
            <div className="text-xs text-gray-500">Done Today</div>
          </div>
        </div>

        {/* Actions */}
        <div className="space-y-2">
          <button
            onClick={handleOpenTasks}
            className="w-full px-4 py-2 bg-accent-blue hover:bg-blue-600 text-white rounded-lg transition-colors font-medium"
          >
            Open Tasks
          </button>
          <div className="flex gap-2">
            <button
              onClick={handleSnooze}
              className="flex-1 px-4 py-2 bg-dark-600 hover:bg-dark-500 text-gray-300 rounded-lg transition-colors text-sm"
            >
              Snooze 1h
            </button>
            <button
              onClick={handleDismiss}
              className="flex-1 px-4 py-2 bg-dark-600 hover:bg-dark-500 text-gray-300 rounded-lg transition-colors text-sm"
            >
              Dismiss
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
