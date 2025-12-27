import { useState, useEffect, useRef, useCallback } from "react";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { invoke } from "@tauri-apps/api/core";

interface ShortcutInputProps {
  onSave: (shortcut: string) => void;
  onCancel: () => void;
}

function ShortcutInput({ onSave, onCancel }: ShortcutInputProps) {
  const [keys, setKeys] = useState<string[]>([]);
  const inputRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Unregister global shortcuts so we can capture key combos
    invoke("unregister_shortcuts").catch(console.error);
    inputRef.current?.focus();
  }, []);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();

    if (e.key === "Escape") {
      onCancel();
      return;
    }

    if (e.key === "Enter" && keys.length > 0) {
      onSave(keys.join("+"));
      return;
    }

    // Build the key combination
    const parts: string[] = [];
    if (e.ctrlKey) parts.push("Ctrl");
    if (e.altKey) parts.push("Alt");
    if (e.shiftKey) parts.push("Shift");
    if (e.metaKey) parts.push("Meta");

    // Add the main key if it's not a modifier
    const key = e.key;
    if (!["Control", "Alt", "Shift", "Meta"].includes(key)) {
      parts.push(key.length === 1 ? key.toUpperCase() : key);
    }

    if (parts.length > 0) {
      setKeys(parts);
    }
  }, [keys, onSave, onCancel]);

  return (
    <div
      ref={inputRef}
      tabIndex={0}
      onKeyDown={handleKeyDown}
      className="px-3 py-2 bg-dark-700 border border-dark-500 rounded-lg text-white text-sm
                 focus:outline-none focus:border-accent-blue focus:ring-1 focus:ring-accent-blue
                 min-w-[140px] text-center cursor-text"
    >
      {keys.length > 0 ? (
        <span className="flex items-center justify-center gap-1">
          {keys.map((k, i) => (
            <span key={i}>
              <kbd className="px-1.5 py-0.5 bg-dark-600 rounded text-xs">{k}</kbd>
              {i < keys.length - 1 && <span className="text-gray-500 mx-0.5">+</span>}
            </span>
          ))}
        </span>
      ) : (
        <span className="text-gray-500">Press keys...</span>
      )}
    </div>
  );
}

interface SettingsDialogProps {
  onClose: () => void;
  onRefreshFromCloud?: () => Promise<boolean>;
}

export function SettingsDialog({ onClose, onRefreshFromCloud }: SettingsDialogProps) {
  const [autoStart, setAutoStart] = useState(false);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [syncStatus, setSyncStatus] = useState<string | null>(null);
  const [editingShortcut, setEditingShortcut] = useState<"quickAdd" | "showList" | null>(null);
  const [quickAddShortcut, setQuickAddShortcut] = useState("Ctrl+Alt+R");
  const [showListShortcut, setShowListShortcut] = useState("Ctrl+Alt+L");

  const registerShortcuts = useCallback((quickAdd: string, showList: string) => {
    invoke("register_shortcuts", { quickAdd, showList }).catch(console.error);
  }, []);

  useEffect(() => {
    isEnabled().then((enabled) => {
      setAutoStart(enabled);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !editingShortcut) onClose();
    };
    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [onClose, editingShortcut]);

  const toggleAutoStart = async () => {
    try {
      if (autoStart) {
        await disable();
        setAutoStart(false);
      } else {
        await enable();
        setAutoStart(true);
      }
    } catch (error) {
      console.error("Failed to toggle autostart:", error);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 animate-fade-in">
      <div className="bg-dark-800 rounded-xl p-6 w-full max-w-sm shadow-2xl animate-slide-up">
        <h2 className="text-lg font-semibold text-white mb-4">Settings</h2>

        <div className="space-y-4">
          {/* Auto-start */}
          <div className="flex items-center justify-between">
            <div>
              <p className="text-white">Start with Windows</p>
              <p className="text-xs text-gray-400">Launch app on system startup</p>
            </div>
            <button
              onClick={toggleAutoStart}
              disabled={loading}
              className={`w-12 h-6 rounded-full transition-colors relative ${
                autoStart ? "bg-accent-blue" : "bg-dark-600"
              }`}
            >
              <span
                className={`absolute top-1 w-4 h-4 bg-white rounded-full transition-transform ${
                  autoStart ? "left-7" : "left-1"
                }`}
              />
            </button>
          </div>

          {/* Hotkey settings */}
          <div className="pt-4 border-t border-dark-600">
            <p className="text-sm text-gray-400 mb-3">Keyboard Shortcuts</p>
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-white text-sm">Quick add</span>
                {editingShortcut === "quickAdd" ? (
                  <ShortcutInput
                    onSave={(shortcut) => {
                      setQuickAddShortcut(shortcut);
                      setEditingShortcut(null);
                      registerShortcuts(shortcut, showListShortcut);
                    }}
                    onCancel={() => {
                      setEditingShortcut(null);
                      registerShortcuts(quickAddShortcut, showListShortcut);
                    }}
                  />
                ) : (
                  <button
                    onClick={() => setEditingShortcut("quickAdd")}
                    className="px-3 py-1.5 bg-dark-700 border border-dark-500 rounded-lg text-white text-sm
                               hover:border-dark-400 transition-colors flex items-center gap-1"
                  >
                    {quickAddShortcut.split("+").map((k, i) => (
                      <span key={i}>
                        <kbd className="px-1.5 py-0.5 bg-dark-600 rounded text-xs">{k}</kbd>
                        {i < quickAddShortcut.split("+").length - 1 && <span className="text-gray-500 mx-0.5">+</span>}
                      </span>
                    ))}
                  </button>
                )}
              </div>
              <div className="flex items-center justify-between">
                <span className="text-white text-sm">Show list</span>
                {editingShortcut === "showList" ? (
                  <ShortcutInput
                    onSave={(shortcut) => {
                      setShowListShortcut(shortcut);
                      setEditingShortcut(null);
                      registerShortcuts(quickAddShortcut, shortcut);
                    }}
                    onCancel={() => {
                      setEditingShortcut(null);
                      registerShortcuts(quickAddShortcut, showListShortcut);
                    }}
                  />
                ) : (
                  <button
                    onClick={() => setEditingShortcut("showList")}
                    className="px-3 py-1.5 bg-dark-700 border border-dark-500 rounded-lg text-white text-sm
                               hover:border-dark-400 transition-colors flex items-center gap-1"
                  >
                    {showListShortcut.split("+").map((k, i) => (
                      <span key={i}>
                        <kbd className="px-1.5 py-0.5 bg-dark-600 rounded text-xs">{k}</kbd>
                        {i < showListShortcut.split("+").length - 1 && <span className="text-gray-500 mx-0.5">+</span>}
                      </span>
                    ))}
                  </button>
                )}
              </div>
            </div>
          </div>

          {/* Cloud Sync */}
          <div className="pt-4 border-t border-dark-600">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-white">Cloud Sync</p>
                <p className="text-xs text-gray-400">Refresh reminders from Google Drive</p>
              </div>
              <button
                onClick={async () => {
                  if (onRefreshFromCloud) {
                    setSyncing(true);
                    setSyncStatus(null);
                    try {
                      const synced = await onRefreshFromCloud();
                      setSyncStatus(synced ? "Synced!" : "Local only");
                    } catch (e) {
                      setSyncStatus("Failed");
                    } finally {
                      setSyncing(false);
                    }
                  }
                }}
                disabled={syncing || !onRefreshFromCloud}
                className="px-3 py-1.5 bg-accent-blue hover:bg-blue-600 disabled:bg-dark-600 disabled:text-gray-500 text-white text-sm rounded-lg transition-colors"
              >
                {syncing ? "Syncing..." : "Sync Now"}
              </button>
            </div>
            {syncStatus && (
              <p className={`text-xs mt-1 ${syncStatus === "Failed" ? "text-red-400" : "text-green-400"}`}>
                {syncStatus}
              </p>
            )}
          </div>

          {/* About */}
          <div className="pt-4 border-t border-dark-600">
            <p className="text-xs text-gray-500">Reminder App v1.0.0</p>
            <p className="text-xs text-gray-500">Built with Tauri + React</p>
          </div>
        </div>

        {/* Close button */}
        <div className="flex justify-end mt-6">
          <button
            onClick={onClose}
            className="px-4 py-2 bg-dark-600 hover:bg-dark-500 text-white rounded-lg transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
