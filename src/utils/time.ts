// Pre-compiled regex patterns for better performance
const RELATIVE_TIME_PATTERN = /\b(in|after)\s+(\d+)\s*(min(?:ute)?s?|hr?s?|hours?|days?|d)\b/i;
const ABSOLUTE_TIME_PATTERN = /\b(at|@)\s*(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\b/i;
const TOMORROW_PATTERN = /\btomorrow\b/i;
const MESSAGE_CLEANUP_PATTERN = /^[-,\s]+|[-,\s]+$/g;

/**
 * Parse natural language time expressions into a Date.
 * Examples: "in 5 minutes", "at 3pm", "tomorrow at 9am", "in 2 hours"
 */
export function parseNaturalTime(text: string): { message: string; dueTime: Date } | null {
  const now = new Date();
  let dueTime: Date | null = null;
  let message = text.trim();

  // Patterns for relative time: "in X minutes/hours/days"
  const relativeMatch = text.match(RELATIVE_TIME_PATTERN);

  if (relativeMatch) {
    const amount = parseInt(relativeMatch[2], 10);
    const unit = relativeMatch[3].toLowerCase();
    dueTime = new Date(now);

    if (unit.startsWith("min")) {
      dueTime.setMinutes(dueTime.getMinutes() + amount);
    } else if (unit.startsWith("h")) {
      dueTime.setHours(dueTime.getHours() + amount);
    } else if (unit.startsWith("d")) {
      dueTime.setDate(dueTime.getDate() + amount);
    }

    message = text.replace(RELATIVE_TIME_PATTERN, "").trim();
  }

  // Patterns for absolute time: "at 3pm", "at 15:30"
  const absoluteMatch = text.match(ABSOLUTE_TIME_PATTERN);

  if (absoluteMatch && !dueTime) {
    let hours = parseInt(absoluteMatch[2], 10);
    const minutes = absoluteMatch[3] ? parseInt(absoluteMatch[3], 10) : 0;
    const period = absoluteMatch[4]?.toLowerCase();

    if (period === "pm" && hours < 12) hours += 12;
    if (period === "am" && hours === 12) hours = 0;

    dueTime = new Date(now);
    dueTime.setHours(hours, minutes, 0, 0);

    // If the time has passed today, set for tomorrow
    if (dueTime <= now) {
      dueTime.setDate(dueTime.getDate() + 1);
    }

    message = text.replace(ABSOLUTE_TIME_PATTERN, "").trim();
  }

  // Handle "tomorrow" prefix
  if (TOMORROW_PATTERN.test(text)) {
    if (dueTime) {
      dueTime.setDate(dueTime.getDate() + 1);
    } else {
      dueTime = new Date(now);
      dueTime.setDate(dueTime.getDate() + 1);
      dueTime.setHours(9, 0, 0, 0); // Default to 9am tomorrow
    }
    message = message.replace(TOMORROW_PATTERN, "").trim();
  }

  // Clean up message
  message = message.replace(MESSAGE_CLEANUP_PATTERN, "").trim();

  // Default to 5 minutes if no time found
  if (!dueTime) {
    dueTime = new Date(now);
    dueTime.setMinutes(dueTime.getMinutes() + 5);
  }

  if (!message) {
    return null;
  }

  return { message, dueTime };
}

/**
 * Get urgency level based on time remaining.
 * Returns: "overdue" | "urgent" | "soon" | "normal"
 */
export function getUrgencyLevel(date: Date): "overdue" | "urgent" | "soon" | "normal" {
  const now = new Date();
  const diffMs = date.getTime() - now.getTime();
  const diffMins = diffMs / 60000;

  if (diffMins <= 0) return "overdue";
  if (diffMins <= 15) return "urgent";  // < 15 minutes
  if (diffMins <= 60) return "soon";    // < 1 hour
  return "normal";
}

/**
 * Format a date for display relative to now.
 */
export function formatRelativeTime(date: Date): string {
  const now = new Date();
  const diffMs = date.getTime() - now.getTime();

  if (diffMs <= 0) {
    return "Due now!";
  }

  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  const parts: string[] = [];

  if (diffDays > 0) {
    parts.push(`${diffDays}d`);
  }
  if (diffHours % 24 > 0) {
    parts.push(`${diffHours % 24}h`);
  }
  if (diffMins % 60 > 0 || parts.length === 0) {
    parts.push(`${diffMins % 60}m`);
  }

  return parts.join(" ");
}

/**
 * Format time with both relative and absolute.
 * Example: "2h 30m (3:45 PM)"
 */
export function formatTimeWithAbsolute(date: Date): { relative: string; absolute: string } {
  const relative = formatRelativeTime(date);
  const absolute = date.toLocaleTimeString("en-US", {
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  });
  return { relative, absolute };
}

/**
 * Format a date for display.
 */
export function formatDueTime(date: Date): string {
  const now = new Date();
  const isToday = date.toDateString() === now.toDateString();

  const tomorrow = new Date(now);
  tomorrow.setDate(tomorrow.getDate() + 1);
  const isTomorrow = date.toDateString() === tomorrow.toDateString();

  const timeStr = date.toLocaleTimeString("en-US", {
    hour: "numeric",
    minute: "2-digit",
    hour12: true,
  });

  if (isToday) {
    return `Today at ${timeStr}`;
  } else if (isTomorrow) {
    return `Tomorrow at ${timeStr}`;
  } else {
    return date.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
    }) + ` at ${timeStr}`;
  }
}
