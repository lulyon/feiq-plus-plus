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

/// Emoji names (same as Rust emoji.rs)
const EMOJI_NAMES: string[] = [
  "微笑", "撇嘴", "色", "发呆", "得意", "流泪", "害羞", "闭嘴",
  "困", "大哭", "尴尬", "发怒", "调皮", "龇牙", "惊讶", "转圈",
  "难过", "酷", "冷汗", "抓狂", "吐", "偷笑", "可爱", "白眼",
  "傲慢", "饥饿", "困", "惊恐", "冷汗", "憨笑", "大兵", "飞吻",
  "奋斗", "咒骂", "疑问", "嘘……", "晕", "折磨", "衰", "骷髅",
  "敲打", "再见", "擦汗", "抠鼻", "鼓掌", "糗大了", "坏笑", "发抖",
  "左哼哼", "右哼哼", "哈欠", "鄙视", "委屈", "快哭了", "阴险", "亲亲",
  "吓", "可怜", "菜刀", "西瓜", "啤酒", "篮球", "乒乓", "跳",
  "咖啡", "吃饭", "猪头", "玫瑰", "枯萎", "示爱", "爱心", "心碎",
  "蛋糕", "闪电", "炸弹", "匕首", "足球", "瓢虫", "大便", "怄火",
  "月亮", "太阳", "礼物", "拥抱", "点赞", "弱", "握手", "胜利",
  "抱拳", "勾引", "拳头", "差劲", "爱你", "no", "ok", "爱情",
];

/// Unicode emoji characters mapped to QQ emoji codes (same order as EMOJI_CODES)
const EMOJI_CHARS: string[] = [
  "😊","😜","😍","😳","😎","😢","😳","🤐",
  "😴","😭","😅","😡","😋","😁","😲","🔄",
  "😔","🆒","😰","🤮","🤭","😏","😍","🙄",
  "😤","🍽️","😴","😱","😅","😄","💂","😘",
  "💪","🤬","🤔","🤫","😵","😖","😞","💀",
  "💥","👋","😓","👃","👏","😅","😏","🥶",
  "😤","😤","🥱","😒","😣","😢","😈","😚",
  "😨","🥺","🔪","🍉","🍺","🏀","🏓","🤸",
  "☕","🍚","🐷","🌹","🥀","💋","❤️","💔",
  "🎂","⚡","💣","🗡️","⚽","🐞","💩","😤",
  "🌙","☀️","🎁","🤗","👍","👎","🤝","✌️",
  "🙏","🫦","👊","👎","🫶","👎","👌","💕",
];

/// Escape HTML entities to prevent XSS
function htmlEscape(text: string): string {
  const replacements: Record<string, string> = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#x27;',
  };
  return text.replace(/[&<>"']/g, (match) => replacements[match]);
}

/** Render text with emoji codes using a two-pass approach:
 *  1. Replace emoji codes with placeholder markers (avoids htmlEscape breaking emoji codes like /<rotate>)
 *  2. HTML-escape the remaining text
 *  3. Replace placeholders with emoji display spans */
function renderText(text: string): string {
  // Sort emoji codes by length descending, so longer codes match first
  // (e.g. /<rotate> before /:r)
  const sortedIndices = EMOJI_CODES
    .map((code, i) => ({ code, i }))
    .sort((a, b) => b.code.length - a.code.length);

  // Pass 1: Replace emoji codes with safe placeholder markers
  let result = text;
  const placeholder = (i: number) => `\x00EMJ${i}\x00`;

  for (const { code, i } of sortedIndices) {
    if (result.includes(code)) {
      result = result.split(code).join(placeholder(i));
    }
  }

  // Pass 2: HTML-escape the remaining text (placeholders are safe ASCII)
  result = htmlEscape(result);

  // Pass 3: Replace placeholders with emoji display elements
  for (let i = 0; i < EMOJI_CODES.length; i++) {
    const ph = placeholder(i);
    if (result.includes(ph)) {
      const ch = EMOJI_CHARS[i] || "";
      const code = EMOJI_CODES[i];
      const title = `${htmlEscape(code)} ${EMOJI_NAMES[i] || ""}`.trim();
      const display = `<span title="${title}" class="emoji-inline" style="font-size:20px;line-height:1;vertical-align:middle">${ch}</span>`;
      result = result.split(ph).join(display);
    }
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
  if (raw.sealed !== undefined && typeof raw.sealed === "object") {
    const inner = raw.sealed as Record<string, unknown>;
    return { type: "sealed", text: String(inner.text || "") };
  }
  // Internally-tagged (serde): {"type": "file", "file_id": 123, "filename": "...", "local_task_id": 456, ...}
  // Fall through for file/image/sealed and unknown types
  return {
    type: String(raw.type || "text"),
    text: typeof raw.text === "string" ? raw.text : String(raw.text || ""),
    filename: String(raw.filename || ""),
    size: Number(raw.size || 0),
    localTaskId: raw.local_task_id !== undefined ? Number(raw.local_task_id) : undefined,
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
            const canDownload = !isSent && !!content.localTaskId;
            return (
              <div
                key={i}
                role={canDownload ? "button" : undefined}
                tabIndex={canDownload ? 0 : undefined}
                onClick={() => {
                  if (canDownload) {
                    handleFileClick(content);
                  }
                }}
                onKeyDown={(e) => {
                  if (canDownload && (e.key === "Enter" || e.key === " ")) {
                    e.preventDefault();
                    handleFileClick(content);
                  }
                }}
                className={`px-3 py-2 rounded-lg text-sm inline-block mb-1
                  ${isSent
                    ? "bg-primary text-white"
                    : "bg-bg text-text hover:bg-surface-alt cursor-pointer group"
                  }`}
                title={canDownload ? "Click to download" : undefined}
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
          if (content.type === "sealed") {
            return (
              <div
                key={i}
                className={`px-3 py-2 rounded-lg text-sm inline-block mb-1
                  ${isSent
                    ? "bg-primary text-white rounded-br-sm"
                    : "bg-bg text-text rounded-bl-sm"
                  }`}
              >
                <span className="mr-1">🔒</span>
                <span
                  dangerouslySetInnerHTML={{
                    __html: renderText(content.text || ""),
                  }}
                />
              </div>
            );
          }
          if (content.type === "image") {
            return (
              <div
                key={i}
                className="px-3 py-2 rounded-lg text-xs italic inline-block mb-1 bg-surface-alt text-text-muted"
              >
                🖼️ Image
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
