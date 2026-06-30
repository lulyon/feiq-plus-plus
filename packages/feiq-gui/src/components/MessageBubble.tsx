import type { Message } from "../stores/messageStore";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

/// Emoji codes mapping (same as Rust emoji.rs)
const EMOJI_CODES: string[] = [
  "/:)", "/:~", "/:*", "/:|", "/8-)", "/:<", "/:$", "/:X",
  "/:Z", "/:'(", "/:-|", "/:@", "/:P", "/:D", "/:O", "/<rotate>",
  "/:(", "/:+", "/:lenhan", "/:Q", "/:T", "/;P", "/;-D", "/;d",
  "/;o", "/:g", "/|-)", "/:!", "/:L", "/:>", "/;bin", "/:fw",
  "/;fd", "/:-S", "/;?", "/;x", "/;@", "/:8", "/;!", "/!!!",
  "/:xx", "/:bye", "/:csweat", "/:knose", "/:applause", "/:cdale",
  "/:huaixiao", "/:shake", "/:lhenhen", "/:rhenhen", "/:yawn",
  "/:snooty", "/:chagrin", "/:kcry", "/:yinxian", "/:qinqin",
  "/:xiaren", "/:kelin", "/:caidao", "/:xig", "/:bj",
  "/:basketball", "/:pingpong", "/:jump", "/:coffee", "/:eat",
  "/:pig", "/:rose", "/:fade", "/:kiss", "/:heart", "/:break",
  "/:cake", "/:shd", "/:bomb", "/:dao", "/:footb", "/:piaocon",
  "/:shit", "/:oh", "/:moon", "/:sun", "/;gift", "/:hug",
  "/:strong", "/;weak", "/:share", "/:shl", "/:baoquan",
  "/:cajole", "/:quantou", "/:chajin", "/:aini", "/:sayno",
  "/:sayok", "/:love",
];

/** Render text with emoji codes replaced by <img> tags */
function renderText(text: string): string {
  let result = text;
  for (let i = 0; i < EMOJI_CODES.length; i++) {
    const code = EMOJI_CODES[i];
    const img = `<img src="emojis/${i + 1}.gif" alt="${code}" class="emoji-inline" style="width:20px;height:20px;vertical-align:middle;display:inline-block" />`;
    result = result.split(code).join(img);
  }
  return result;
}

/** Normalized content with type field resolved */
interface NormalizedContent {
  type: string;
  text?: string;
  format?: string;
  filename?: string;
  size?: number;
  localTaskId?: number;
}

/** Normalize Content from either externally-tagged (Rust serde) or internally-tagged (frontend) format */
function normalizeContent(raw: Record<string, unknown>): NormalizedContent {
  // Externally-tagged: {"text": {"text": "Hello", "format": ""}}
  if (raw.text !== undefined && typeof raw.text === "object") {
    const inner = raw.text as Record<string, unknown>;
    return { type: "text", text: String(inner.text || ""), format: String(inner.format || "") };
  }
  if (raw.knock !== undefined) return { type: "knock" };
  if (raw.file !== undefined && typeof raw.file === "object") {
    const inner = raw.file as Record<string, unknown>;
    return {
      type: "file",
      filename: String(inner.filename || ""),
      size: Number(inner.size || 0),
      localTaskId: inner.local_task_id !== undefined ? Number(inner.local_task_id) : undefined,
    };
  }
  if (raw.image !== undefined) return { type: "image" };
  // Already internally-tagged or unknown — return as-is with defaults
  return {
    type: String(raw.type || "text"),
    text: typeof raw.text === "string" ? raw.text : String(raw.text || ""),
    filename: String(raw.filename || ""),
    size: Number(raw.size || 0),
  };
}

/** Handle click on a received file bubble: open save dialog and download */
async function handleFileClick(content: NormalizedContent) {
  if (!content.localTaskId) {
    console.warn("File click: no localTaskId, cannot download");
    return;
  }
  try {
    const savePath = await save({
      defaultPath: content.filename || "download",
      filters: [{ name: "All Files", extensions: ["*"] }],
    });
    if (!savePath) return; // User canceled
    await invoke("download_file", {
      taskId: content.localTaskId,
      savePath,
    });
  } catch (e) {
    console.error("File download failed:", e);
  }
}

export function MessageBubble({
  message,
  showFromNameAlways,
}: {
  message: Message;
  showFromNameAlways?: boolean;
}) {
  const isSent = message.direction === "sent";
  const time = new Date(message.timestamp).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });

  const showFromName = showFromNameAlways || !isSent;
  const fromLabel = isSent ? "Me" : message.fromName;

  return (
    <div className={`flex ${isSent ? "justify-end" : "justify-start"}`}>
      <div className={`max-w-[70%] ${isSent ? "order-1" : ""}`}>
        {showFromName && (
          <div className="text-xs text-text-muted mb-0.5 ml-1">
            {fromLabel}
          </div>
        )}

        {message.contents.map((rawContent, i) => {
          const content = normalizeContent(rawContent as unknown as Record<string, unknown>);
          if (content.type === "text") {
            return (
              <div
                key={i}
                className={`px-3 py-2 rounded-lg text-sm inline-block mb-1
                  ${isSent
                    ? "bg-primary text-white rounded-br-sm"
                    : "bg-bg text-text rounded-bl-sm"
                  }`}
                dangerouslySetInnerHTML={{
                  __html: renderText(content.text || ""),
                }}
              />
            );
          }
          if (content.type === "knock") {
            return (
              <div
                key={i}
                className="px-3 py-1.5 rounded-lg text-xs text-text-muted bg-surface-alt italic animate-shake"
              >
                {isSent ? "You sent a window shake" : "Window shake!"}
              </div>
            );
          }
          if (content.type === "file") {
            return (
              <div
                key={i}
                onClick={() => {
                  if (!isSent && content.localTaskId) {
                    handleFileClick(content);
                  }
                }}
                className={`px-3 py-2 rounded-lg text-sm inline-block mb-1
                  ${isSent
                    ? "bg-primary text-white"
                    : "bg-bg text-text hover:bg-surface-alt cursor-pointer group"
                  }`}
                title={!isSent && content.localTaskId ? "Click to download" : undefined}
              >
                <span className="group-hover:underline">
                  📎 {content.filename || "File"} ({content.size ? formatSize(content.size) : "?"})
                </span>
                {!isSent && content.localTaskId && (
                  <span className="ml-2 text-xs opacity-0 group-hover:opacity-60 transition-opacity">
                    Click to download
                  </span>
                )}
              </div>
            );
          }
          return null;
        })}

        <div className={`text-xs text-text-muted mt-0.5 ${isSent ? "text-right mr-1" : "ml-1"}`}>
          {time}
        </div>
      </div>
    </div>
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)}GB`;
}
