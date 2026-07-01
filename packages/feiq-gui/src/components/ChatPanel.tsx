import { useEffect, useState, Fragment, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useContactStore } from "../stores/contactStore";
import { useGroupStore } from "../stores/groupStore";
import { useMessageStore, type Message, type Content } from "../stores/messageStore";
import { MessageBubble } from "./MessageBubble";
import { InputArea } from "./InputArea";
import { FileTransferPanel } from "./FileTransferPanel";
import { Search, X, Send, MessageSquare, Users, Loader2 } from "lucide-react";
import type { Group } from "../stores/groupStore";

/** Rust MessageRecord shape from get_chat_history */
interface MessageRecord {
  id: number;
  contact_ip: string;
  contact_name: string;
  direction: number; // 0 = sent, 1 = received
  content_json: string;
  timestamp: number;
}

/** Return a human-readable date label for the given timestamp */
function getDateLabel(timestamp: number): string {
  const date = new Date(timestamp);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  const msgDate = new Date(date.getFullYear(), date.getMonth(), date.getDate());

  const diffDays = Math.floor(
    (today.getTime() - msgDate.getTime()) / (1000 * 60 * 60 * 24)
  );

  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  if (diffDays < 7) {
    const days = [
      "Sunday", "Monday", "Tuesday", "Wednesday",
      "Thursday", "Friday", "Saturday",
    ];
    return days[date.getDay()];
  }
  return date.toLocaleDateString();
}

/** A styled date separator shown between messages on different days */
function DateSeparator({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-3 py-2">
      <div className="flex-1 h-px bg-border" />
      <span className="text-xs text-text-muted flex-shrink-0">{label}</span>
      <div className="flex-1 h-px bg-border" />
    </div>
  );
}

/** Parse JSON content array from Rust (externally-tagged serde enum) into Content[] */
function parseContentJson(json: string): Content[] {
  try {
    const parsed = JSON.parse(json);
    if (Array.isArray(parsed)) {
      return parsed.map((item: Record<string, unknown>) => {
        // Handle externally-tagged format: {"text": {"text": "Hello", "format": ""}}
        if (item.text !== undefined && typeof item.text === "object") {
          const inner = item.text as Record<string, unknown>;
          return { type: "text" as const, text: String(inner.text || "") };
        }
        if (item.knock !== undefined) {
          return { type: "knock" as const };
        }
        if (item.file !== undefined && typeof item.file === "object") {
          const inner = item.file as Record<string, unknown>;
          return {
            type: "file" as const,
            filename: String(inner.filename || ""),
            size: Number(inner.size || 0),
          };
        }
        // Handle internally-tagged format (frontend-sent): {"type": "text", "text": "Hello"}
        if (item.type === "text") {
          return { type: "text" as const, text: String(item.text || "") };
        }
        if (item.type === "knock") {
          return { type: "knock" as const };
        }
        if (item.type === "file") {
          return {
            type: "file" as const,
            filename: String(item.filename || ""),
            size: Number(item.size || 0),
          };
        }
        return { type: "text" as const, text: String(item.text || "") };
      });
    }
  } catch {
    // If parsing fails, treat as plain text
  }
  return [{ type: "text" as const, text: json }];
}

/** Extract a plain text preview from content_json (JSON array of Content objects) */
function extractPreviewText(contentJson: string): string {
  try {
    const parsed = JSON.parse(contentJson);
    if (Array.isArray(parsed) && parsed.length > 0) {
      for (const item of parsed) {
        // Handle externally-tagged format: {"text": {"text": "Hello"}}
        if (typeof item.text === "object" && item.text?.text) {
          return String(item.text.text).slice(0, 100);
        }
        // Handle internally-tagged format: {"type": "text", "text": "Hello"}
        if (item.type === "text" && item.text) {
          return String(item.text).slice(0, 100);
        }
        // Handle file content
        if (item.file !== undefined || item.type === "file") {
          return "[File]";
        }
        if (item.knock !== undefined || item.type === "knock") {
          return "[Knock]";
        }
      }
      // Fallback to first item string
      return String(parsed[0]?.text || JSON.stringify(parsed[0])).slice(0, 100);
    }
    return String(parsed).slice(0, 100);
  } catch {
    return contentJson.slice(0, 100);
  }
}

export function ChatPanel() {
  const selectedIp = useContactStore((s) => s.selectedIp);
  const contacts = useContactStore((s) => s.contacts);
  const messagesByIp = useMessageStore((s) => s.messagesByIp);
  const prependMessages = useMessageStore((s) => s.prependMessages);
  const hasHistory = useMessageStore((s) => s.hasHistory);
  const setHasHistory = useMessageStore((s) => s.setHasHistory);
  const loadingHistory = useMessageStore((s) => s.loadingHistory);
  const setLoadingHistory = useMessageStore((s) => s.setLoadingHistory);
  const historyOffset = useMessageStore((s) => s.historyOffset);
  const setHistoryOffset = useMessageStore((s) => s.setHistoryOffset);

  const groups = useGroupStore((s) => s.groups);
  const selectedGroupName = useGroupStore((s) => s.selectedGroupName);

  // ─── Group Chat Mode ─────────────────────────────────────────
  const selectedGroup = selectedGroupName
    ? groups.find((g) => g.name === selectedGroupName) || null
    : null;

  if (selectedGroup) {
    return <GroupChatPanel group={selectedGroup} />;
  }

  // ─── Direct Chat Mode (existing logic) ───────────────────────
  const fellow = contacts.find((c) => c.ip === selectedIp);
  const displayName = fellow
    ? fellow.alias || fellow.name || fellow.pc_name || fellow.ip
    : "";
  const messages = selectedIp ? messagesByIp[selectedIp] || [] : [];

  // Track if there are more history pages to load (per contact)
  const hasMoreRef = useRef<Record<string, boolean>>({});
  // Ref for the scroll container
  const scrollRef = useRef<HTMLDivElement>(null);

  const PAGE_SIZE = 100;

  // Alias inline editing state
  const [editingAlias, setEditingAlias] = useState(false);
  const [aliasDraft, setAliasDraft] = useState("");

  const handleAliasDoubleClick = () => {
    setAliasDraft(displayName === fellow?.ip ? "" : displayName);
    setEditingAlias(true);
  };

  const handleAliasSave = async () => {
    if (!selectedIp) return;
    try {
      await invoke("set_alias", { ip: selectedIp, alias: aliasDraft });
    } catch (e) {
      console.error("set_alias failed:", e);
    }
    setEditingAlias(false);
  };

  // ─── Message Search State ─────────────────────────────────────
  const [searchMode, setSearchMode] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<MessageRecord[]>([]);
  const [searching, setSearching] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const selectContact = useContactStore((s) => s.selectContact);

  const handleToggleSearch = () => {
    if (searchMode) {
      // Exit search mode
      setSearchMode(false);
      setSearchQuery("");
      setSearchResults([]);
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
        debounceRef.current = null;
      }
    } else {
      setSearchMode(true);
      // Focus input after render
      setTimeout(() => searchInputRef.current?.focus(), 0);
    }
  };

  const handleSearchInput = (value: string) => {
    setSearchQuery(value);
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }
    if (!value.trim()) {
      setSearchResults([]);
      return;
    }
    debounceRef.current = setTimeout(async () => {
      setSearching(true);
      try {
        const results = await invoke<MessageRecord[]>("search_chat_history", {
          query: value.trim(),
          limit: 50,
        });
        setSearchResults(results);
      } catch (e) {
        console.error("search_chat_history failed:", e);
      } finally {
        setSearching(false);
      }
    }, 300);
  };

  const handleSearchResultClick = (record: MessageRecord) => {
    selectContact(record.contact_ip);
    setSearchMode(false);
    setSearchQuery("");
    setSearchResults([]);
  };

  const handleSearchKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      handleToggleSearch();
    }
  };

  /** Load the next page of older history, prepend to the store */
  const loadMoreHistory = useCallback(async () => {
    if (!selectedIp) return;
    const offset = historyOffset[selectedIp] || 0;
    setLoadingHistory(selectedIp, true);

    try {
      const records = await invoke<MessageRecord[]>("get_chat_history", {
        ip: selectedIp,
        offset,
        limit: PAGE_SIZE,
      });

      if (records.length < PAGE_SIZE) {
        hasMoreRef.current[selectedIp] = false;
      } else {
        hasMoreRef.current[selectedIp] = true;
      }

      if (records.length > 0) {
        const msgs: Message[] = records.map((r) => ({
          id: r.id,
          fromIp: r.contact_ip,
          fromName: r.contact_name,
          contents: parseContentJson(r.content_json),
          timestamp: r.timestamp,
          direction: r.direction === 0 ? "sent" : "received",
        }));
        prependMessages(selectedIp, msgs);
        setHistoryOffset(selectedIp, offset + records.length);
      }

      // Mark history as fully loaded when fewer results than page size
      if (records.length < PAGE_SIZE) {
        setHasHistory(selectedIp, true);
      }
    } catch (e) {
      console.error("Failed to load more history:", e);
    } finally {
      setLoadingHistory(selectedIp, false);
    }
  }, [selectedIp, historyOffset, setLoadingHistory, setHistoryOffset, prependMessages, setHasHistory]);

  /** Scroll handler: detect when user scrolls to top for lazy loading */
  const handleScroll = useCallback(() => {
    if (!selectedIp) return;
    const container = scrollRef.current;
    if (!container) return;

    const isLoading = loadingHistory[selectedIp];
    const hasMore = hasMoreRef.current[selectedIp];

    if (container.scrollTop === 0 && !isLoading && hasMore) {
      const prevScrollHeight = container.scrollHeight;
      loadMoreHistory().then(() => {
        requestAnimationFrame(() => {
          if (scrollRef.current) {
            scrollRef.current.scrollTop = scrollRef.current.scrollHeight - prevScrollHeight;
          }
        });
      });
    }
  }, [selectedIp, loadingHistory, loadMoreHistory]);

  // Load chat history when switching contacts (initial page)
  useEffect(() => {
    if (!selectedIp) return;

    // Reset pagination state for this contact on first load
    if (!hasHistory[selectedIp]) {
      setHistoryOffset(selectedIp, 0);
      hasMoreRef.current[selectedIp] = true;

      invoke<MessageRecord[]>("get_chat_history", {
        ip: selectedIp,
        offset: 0,
        limit: PAGE_SIZE,
      })
        .then((records) => {
          if (records.length < PAGE_SIZE) {
            hasMoreRef.current[selectedIp] = false;
          }
          if (records.length === 0) {
            setHasHistory(selectedIp, true);
            return;
          }
          const msgs: Message[] = records.map((r) => ({
            id: r.id,
            fromIp: r.contact_ip,
            fromName: r.contact_name,
            contents: parseContentJson(r.content_json),
            timestamp: r.timestamp,
            direction: r.direction === 0 ? "sent" : "received",
          }));
          prependMessages(selectedIp, msgs);
          setHistoryOffset(selectedIp, records.length);
          if (records.length < PAGE_SIZE) {
            setHasHistory(selectedIp, true);
          }
        })
        .catch((e) => console.error("Failed to load chat history:", e));
    }
  }, [selectedIp]);

  if (!selectedIp || !fellow) {
    return (
      <div className="flex-1 flex items-center justify-center bg-surface">
        <div className="text-center text-text-muted">
          <MessageSquare className="w-16 h-16 mx-auto mb-4 opacity-30" />
          <p className="text-lg">feiq++</p>
          <p className="text-sm mt-1">Select a contact or group to start chatting</p>
          <p className="text-xs mt-4 text-text-muted">
            LAN instant messaging · IP Messenger compatible
          </p>
        </div>
      </div>
    );
  }

  const isLoading = selectedIp ? loadingHistory[selectedIp] : false;

  return (
    <div className="flex-1 flex flex-col bg-surface">
      {/* Chat Header */}
      <div className="px-4 py-3 border-b border-border flex items-center gap-3 bg-surface-alt">
        {searchMode ? (
          /* ─── Search Mode Header ─── */
          <>
            <input
              ref={searchInputRef}
              type="text"
              value={searchQuery}
              onChange={(e) => handleSearchInput(e.target.value)}
              onKeyDown={handleSearchKeyDown}
              placeholder="Search all chat history..."
              className="flex-1 text-sm px-2 py-1.5 rounded-md border border-border
                         focus:outline-none focus:border-primary bg-surface"
            />
            {searching && <Loader2 className="w-4 h-4 text-text-muted animate-spin flex-shrink-0" />}
            {!searching && searchQuery && searchResults.length > 0 && (
              <span className="text-xs text-text-muted flex-shrink-0">{searchResults.length} result{searchResults.length !== 1 ? "s" : ""}</span>
            )}
            <button
              onClick={handleToggleSearch}
              className="w-7 h-7 flex items-center justify-center rounded-md hover:bg-surface-alt text-text-muted cursor-pointer flex-shrink-0"
              title="Close search"
            >
              <X className="w-4 h-4" />
            </button>
          </>
        ) : (
          /* ─── Normal Mode Header ─── */
          <>
            <span
              className={`w-2.5 h-2.5 rounded-full ${
                fellow.online ? "bg-online" : "bg-offline"
              }`}
            />
            <div className="flex-1 min-w-0">
              {editingAlias ? (
                <input
                  type="text"
                  value={aliasDraft}
                  onChange={(e) => setAliasDraft(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleAliasSave();
                    if (e.key === "Escape") setEditingAlias(false);
                  }}
                  onBlur={() => setEditingAlias(false)}
                  className="text-sm font-semibold w-full px-1 py-0.5 border border-border rounded
                             focus:outline-none focus:border-primary bg-surface"
                  autoFocus
                />
              ) : (
                <div
                  className="text-sm font-semibold text-text cursor-pointer"
                  onDoubleClick={handleAliasDoubleClick}
                  title="Double-click to edit alias"
                >
                  {displayName}
                </div>
              )}
              {fellow.signature && !editingAlias && (
                <div className="text-xs text-text-muted truncate">{fellow.signature}</div>
              )}
              <div className="text-xs text-text-muted">
                {fellow.online ? "Online" : "Offline"} · {fellow.ip}
              </div>
            </div>
            <button
              onClick={handleToggleSearch}
              className="w-7 h-7 flex items-center justify-center rounded-md hover:bg-surface-alt text-text-muted cursor-pointer flex-shrink-0"
              title="Search chat history"
            >
              <Search className="w-4 h-4" />
            </button>
          </>
        )}
      </div>

      {/* ─── Search Results ─── */}
      {searchMode && searchResults.length > 0 && (
        <div className="border-b border-border max-h-60 overflow-y-auto bg-surface z-10">
          {searchResults.map((record) => {
            const resultContact = contacts.find((c) => c.ip === record.contact_ip);
            const displayName = resultContact
              ? resultContact.alias || resultContact.name || resultContact.pc_name || resultContact.ip
              : record.contact_name || record.contact_ip;
            const previewText = extractPreviewText(record.content_json);
            const time = new Date(record.timestamp);
            const timeStr = time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
            return (
              <div
                key={record.id}
                onClick={() => handleSearchResultClick(record)}
                className="px-4 py-2 hover:bg-surface-alt cursor-pointer border-b border-border last:border-b-0"
              >
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium text-text truncate">{displayName}</span>
                  <span className="text-xs text-text-muted ml-2 flex-shrink-0">{timeStr}</span>
                </div>
                <div className="text-xs text-text-muted truncate mt-0.5">{previewText}</div>
              </div>
            );
          })}
        </div>
      )}

      {/* Active file transfers */}
      <FileTransferPanel />

      {/* Messages with infinite scroll */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto px-4 py-3"
      >
        {/* Loading indicator at top */}
        {isLoading && (
          <div className="flex items-center justify-center py-2 text-text-muted text-xs">
            <Loader2 className="w-3 h-3 mr-1 animate-spin" />
            Loading older messages...
          </div>
        )}

        {messages.length === 0 && !isLoading ? (
          <div className="text-center text-text-muted text-sm mt-8">
            No messages yet. Say hello!
          </div>
        ) : (
          <div className="space-y-2">
            {(() => {
              let lastDate: string | null = null;
              return messages.map((msg, i) => {
                const msgDate = getDateLabel(msg.timestamp);
                const showSeparator = msgDate !== lastDate;
                lastDate = msgDate;
                return (
                  <Fragment key={`msg-${msg.id ?? `${msg.timestamp}-${i}`}`}>
                    {showSeparator && <DateSeparator label={msgDate} />}
                    <MessageBubble message={msg} />
                  </Fragment>
                );
              });
            })()}
          </div>
        )}
      </div>

      {/* Input Area */}
      <InputArea fellowIp={fellow.ip} />
    </div>
  );
}

// ─── Group Chat Panel ─────────────────────────────────────────────

/** Group chat panel used when a group is selected in the sidebar */
function GroupChatPanel({ group }: { group: Group }) {
  const groupKey = `group:${group.name}`;
  const [showMembers, setShowMembers] = useState(false);

  const contacts = useContactStore((s) => s.contacts);
  const messagesByIp = useMessageStore((s) => s.messagesByIp);
  const prependMessages = useMessageStore((s) => s.prependMessages);
  const addMessage = useMessageStore((s) => s.addMessage);

  const groupMessages = messagesByIp[groupKey] || [];
  const [loadedHistory, setLoadedHistory] = useState(false);

  // Load group chat history on selection
  useEffect(() => {
    if (loadedHistory) return;
    setLoadedHistory(true);

    invoke<MessageRecord[]>("get_chat_history", {
      ip: groupKey,
      offset: 0,
      limit: 100,
    })
      .then((records) => {
        if (records.length === 0) return;
        const msgs: Message[] = records
          .map((r) => ({
            id: r.id,
            fromIp: r.contact_ip,
            fromName: r.contact_name,
            contents: parseContentJson(r.content_json),
            timestamp: r.timestamp,
            direction: (r.direction === 0 ? "sent" : "received") as "sent" | "received",
          }));
        prependMessages(groupKey, msgs);
      })
      .catch((e) => console.error("Failed to load group chat history:", e));
  }, [group.name]);

  const handleSend = (text: string) => {
    const trimmed = text.trim();
    if (!trimmed) return;

    const prefixed = `[${group.name}] ${trimmed}`;

    addMessage(groupKey, {
      fromIp: "self",
      fromName: "Me",
      contents: [{ type: "text", text: prefixed }],
      timestamp: Date.now(),
      direction: "sent",
    });

    invoke("send_group_text", { groupName: group.name, text: trimmed })
      .catch((e) => console.error("send_group_text failed:", e));
  };

  const [inputValue, setInputValue] = useState("");

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend(inputValue);
      setInputValue("");
    }
  };

  return (
    <div className="flex-1 flex flex-col bg-surface">
      {/* Group Header */}
      <div className="px-4 py-3 border-b border-border flex items-center gap-3 bg-surface-alt">
        <Users className="w-5 h-5 text-primary flex-shrink-0" />
        <div className="flex-1 min-w-0">
          <div className="text-sm font-semibold text-text">
            {group.name}
          </div>
          <button
            onClick={() => setShowMembers(!showMembers)}
            className="text-xs text-text-muted hover:text-primary transition-colors cursor-pointer"
          >
            {group.memberIps.length} member{group.memberIps.length !== 1 ? "s" : ""}
            {showMembers ? " (hide)" : ""}
          </button>
        </div>
      </div>

      {/* Member list dropdown */}
      {showMembers && (
        <div className="px-4 py-2 bg-surface-alt border-b border-border">
          <div className="text-xs text-text-muted mb-1">Members:</div>
          <div className="flex flex-wrap gap-1">
            {group.memberIps.map((ip) => {
              const member = contacts.find((c) => c.ip === ip);
              const memberName = member
                ? member.alias || member.name || member.pc_name || ip
                : ip;
              return (
                <span
                  key={ip}
                  className="inline-flex items-center gap-1 px-2 py-0.5 bg-surface
                             border border-border rounded-full text-xs text-text-muted"
                >
                  <span
                    className={`w-1.5 h-1.5 rounded-full ${
                      member?.online ? "bg-online" : "bg-offline"
                    }`}
                  />
                  {memberName}
                </span>
              );
            })}
          </div>
        </div>
      )}

      {/* Group Messages */}
      <div className="flex-1 overflow-y-auto px-4 py-3">
        {groupMessages.length === 0 ? (
          <div className="text-center text-text-muted text-sm mt-8">
            No messages in this group yet. Send a message to all members!
          </div>
        ) : (
          <div className="space-y-2">
            {(() => {
              let lastDate: string | null = null;
              return groupMessages.map((msg, i) => {
                const msgDate = getDateLabel(msg.timestamp);
                const showSeparator = msgDate !== lastDate;
                lastDate = msgDate;
                return (
                  <Fragment key={`msg-${msg.id ?? `${msg.timestamp}-${i}`}`}>
                    {showSeparator && <DateSeparator label={msgDate} />}
                    <MessageBubble message={msg} showFromNameAlways />
                  </Fragment>
                );
              });
            })()}
          </div>
        )}
      </div>

      {/* Group Input */}
      <div className="border-t border-border px-4 py-3 bg-surface-alt">
        <div className="flex items-end gap-2">
          <textarea
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={`Type a message to ${group.name}... (Enter to send)`}
            rows={2}
            className="flex-1 resize-none text-sm px-3 py-2 rounded-lg border border-border
                       focus:outline-none focus:border-primary bg-surface"
          />
          <button
            onClick={() => {
              handleSend(inputValue);
              setInputValue("");
            }}
            disabled={!inputValue.trim()}
            className="flex-shrink-0 w-10 h-10 flex items-center justify-center
                       rounded-lg bg-primary text-white hover:bg-primary-hover
                       disabled:bg-offline transition-colors cursor-pointer"
          >
            <Send className="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>
  );
}
