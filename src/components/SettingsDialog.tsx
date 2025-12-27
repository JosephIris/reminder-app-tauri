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
  onCheckForUpdates?: () => Promise<"available" | "up-to-date" | "error">;
  checkingForUpdates?: boolean;
}

export function SettingsDialog({ onClose, onRefreshFromCloud, onCheckForUpdates, checkingForUpdates }: SettingsDialogProps) {
  const [autoStart, setAutoStart] = useState(false);
  const [loading, setLoading] = useState(true);
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [syncStatus, setSyncStatus] = useState<string | null>(null);
  const [editingShortcut, setEditingShortcut] = useState<"quickAdd" | "showList" | null>(null);
  const [quickAddShortcut, setQuickAddShortcut] = useState("Ctrl+Alt+R");
  const [showListShortcut, setShowListShortcut] = useState("Ctrl+Alt+L");

  // OAuth state
  const [hasCredentials, setHasCredentials] = useState(false);
  const [isLoggedIn, setIsLoggedIn] = useState(false);
  const [showCredentialsForm, setShowCredentialsForm] = useState(false);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [oauthLoading, setOauthLoading] = useState(false);
  const [oauthError, setOauthError] = useState<string | null>(null);

  const registerShortcuts = useCallback((quickAdd: string, showList: string) => {
    invoke("register_shortcuts", { quickAdd, showList }).catch(console.error);
  }, []);

  // Load OAuth status
  useEffect(() => {
    invoke<[boolean, boolean]>("get_oauth_status")
      .then(([hasCreds, loggedIn]) => {
        setHasCredentials(hasCreds);
        setIsLoggedIn(loggedIn);
      })
      .catch(console.error);
  }, []);

  const handleSaveCredentials = async () => {
    if (!clientId.trim() || !clientSecret.trim()) return;

    setOauthLoading(true);
    setOauthError(null);
    try {
      await invoke("save_oauth_credentials", {
        clientId: clientId.trim(),
        clientSecret: clientSecret.trim(),
      });
      setHasCredentials(true);
      setShowCredentialsForm(false);
      setClientId("");
      setClientSecret("");
    } catch (e) {
      setOauthError(String(e));
    } finally {
      setOauthLoading(false);
    }
  };

  const handleLogin = async () => {
    setOauthLoading(true);
    setOauthError(null);
    try {
      await invoke("start_oauth_flow");
      setIsLoggedIn(true);
      // Refresh reminders from cloud after successful login
      if (onRefreshFromCloud) {
        await onRefreshFromCloud();
      }
    } catch (e) {
      setOauthError(String(e));
    } finally {
      setOauthLoading(false);
    }
  };

  const handleDisconnect = async () => {
    setOauthLoading(true);
    try {
      await invoke("disconnect_drive");
      setIsLoggedIn(false);
    } catch (e) {
      setOauthError(String(e));
    } finally {
      setOauthLoading(false);
    }
  };

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

          {/* Google Drive Sync */}
          <div className="pt-4 border-t border-dark-600">
            <p className="text-sm text-gray-400 mb-3">Google Drive Sync</p>

            {/* Status indicator */}
            <div className="flex items-center gap-2 mb-3">
              <div className={`w-2 h-2 rounded-full ${isLoggedIn ? "bg-green-500" : "bg-gray-500"}`} />
              <span className="text-sm text-gray-300">
                {isLoggedIn ? "Connected to Google Drive" : hasCredentials ? "Not logged in" : "Not configured"}
              </span>
            </div>

            {/* Credentials form */}
            {showCredentialsForm && (
              <div className="space-y-2 mb-3 p-3 bg-dark-700 rounded-lg">
                <p className="text-xs text-gray-400 mb-2">
                  Enter your GCP OAuth credentials:
                </p>
                <input
                  type="text"
                  placeholder="Client ID"
                  value={clientId}
                  onChange={(e) => setClientId(e.target.value)}
                  className="w-full px-3 py-2 bg-dark-600 border border-dark-500 rounded text-white text-sm
                             focus:outline-none focus:border-accent-blue"
                />
                <input
                  type="password"
                  placeholder="Client Secret"
                  value={clientSecret}
                  onChange={(e) => setClientSecret(e.target.value)}
                  className="w-full px-3 py-2 bg-dark-600 border border-dark-500 rounded text-white text-sm
                             focus:outline-none focus:border-accent-blue"
                />
                <div className="flex gap-2 mt-2">
                  <button
                    onClick={handleSaveCredentials}
                    disabled={oauthLoading || !clientId.trim() || !clientSecret.trim()}
                    className="px-3 py-1.5 bg-accent-blue hover:bg-blue-600 disabled:bg-dark-600 text-white text-sm rounded transition-colors"
                  >
                    Save
                  </button>
                  <button
                    onClick={() => setShowCredentialsForm(false)}
                    className="px-3 py-1.5 bg-dark-600 hover:bg-dark-500 text-white text-sm rounded transition-colors"
                  >
                    Cancel
                  </button>
                </div>
              </div>
            )}

            {/* Action buttons */}
            <div className="flex flex-wrap gap-2">
              {!hasCredentials && !showCredentialsForm && (
                <button
                  onClick={() => setShowCredentialsForm(true)}
                  className="px-3 py-1.5 bg-dark-600 hover:bg-dark-500 text-white text-sm rounded-lg transition-colors"
                >
                  Setup Credentials
                </button>
              )}

              {hasCredentials && !isLoggedIn && (
                <button
                  onClick={handleLogin}
                  disabled={oauthLoading}
                  className="px-3 py-1.5 bg-accent-blue hover:bg-blue-600 disabled:bg-dark-600 text-white text-sm rounded-lg transition-colors"
                >
                  {oauthLoading ? "Connecting..." : "Connect to Google"}
                </button>
              )}

              {isLoggedIn && (
                <>
                  <button
                    onClick={async () => {
                      if (onRefreshFromCloud) {
                        setSyncing(true);
                        setSyncStatus(null);
                        try {
                          const synced = await onRefreshFromCloud();
                          setSyncStatus(synced ? "Synced!" : "Failed");
                        } catch (e) {
                          setSyncStatus("Failed");
                        } finally {
                          setSyncing(false);
                        }
                      }
                    }}
                    disabled={syncing}
                    className="px-3 py-1.5 bg-accent-blue hover:bg-blue-600 disabled:bg-dark-600 text-white text-sm rounded-lg transition-colors"
                  >
                    {syncing ? "Syncing..." : "Sync Now"}
                  </button>
                  <button
                    onClick={handleDisconnect}
                    disabled={oauthLoading}
                    className="px-3 py-1.5 bg-dark-600 hover:bg-dark-500 text-red-400 text-sm rounded-lg transition-colors"
                  >
                    Disconnect
                  </button>
                </>
              )}

              {hasCredentials && !showCredentialsForm && (
                <button
                  onClick={async () => {
                    // Load existing credentials when editing
                    try {
                      const [existingClientId, existingClientSecret] = await invoke<[string, string]>("get_oauth_credentials");
                      setClientId(existingClientId);
                      setClientSecret(existingClientSecret);
                    } catch (e) {
                      // Ignore if credentials can't be loaded
                    }
                    setShowCredentialsForm(true);
                  }}
                  className="px-3 py-1.5 bg-dark-700 hover:bg-dark-600 text-gray-400 text-sm rounded-lg transition-colors"
                >
                  Edit Credentials
                </button>
              )}
            </div>

            {/* Status messages */}
            {syncStatus && (
              <p className={`text-xs mt-2 ${syncStatus === "Failed" ? "text-red-400" : "text-green-400"}`}>
                {syncStatus}
              </p>
            )}
            {oauthError && (
              <p className="text-xs mt-2 text-red-400">{oauthError}</p>
            )}
          </div>

          {/* About */}
          <div className="pt-4 border-t border-dark-600">
            <p className="text-xs text-gray-500 mb-2">Reminder App v1.1.3</p>
            <div className="flex items-center gap-2">
              <button
                onClick={async () => {
                  if (onCheckForUpdates) {
                    setUpdateStatus(null);
                    const result = await onCheckForUpdates();
                    if (result === "up-to-date") {
                      setUpdateStatus("You're up to date!");
                    } else if (result === "error") {
                      setUpdateStatus("Failed to check for updates");
                    }
                    // "available" case is handled by the update banner in App.tsx
                  }
                }}
                disabled={checkingForUpdates}
                className="px-3 py-1.5 bg-dark-600 hover:bg-dark-500 disabled:bg-dark-700 text-white text-sm rounded-lg transition-colors"
              >
                {checkingForUpdates ? "Checking..." : "Check for Updates"}
              </button>
              {updateStatus && (
                <span className={`text-xs ${updateStatus.includes("Failed") ? "text-red-400" : "text-green-400"}`}>
                  {updateStatus}
                </span>
              )}
            </div>
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
