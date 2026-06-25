import type { Message } from "../stores/messageStore";

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

export function MessageBubble({ message }: { message: Message }) {
  const isSent = message.direction === "sent";
  const time = new Date(message.timestamp).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });

  return (
    <div className={`flex ${isSent ? "justify-end" : "justify-start"}`}>
      <div className={`max-w-[70%] ${isSent ? "order-1" : ""}`}>
        {!isSent && (
          <div className="text-xs text-gray-400 mb-0.5 ml-1">
            {message.fromName}
          </div>
        )}

        {message.contents.map((content, i) => {
          if (content.type === "text") {
            return (
              <div
                key={i}
                className={`px-3 py-2 rounded-lg text-sm inline-block mb-1
                  ${isSent
                    ? "bg-blue-500 text-white rounded-br-sm"
                    : "bg-gray-100 text-gray-800 rounded-bl-sm"
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
                className="px-3 py-1.5 rounded-lg text-xs text-gray-500 bg-gray-50 italic animate-shake"
              >
                {isSent ? "You sent a window shake" : "Window shake!"}
              </div>
            );
          }
          if (content.type === "file") {
            return (
              <div
                key={i}
                className={`px-3 py-2 rounded-lg text-sm inline-block mb-1 cursor-pointer
                  ${isSent ? "bg-blue-500 text-white" : "bg-gray-100 text-gray-800 hover:bg-gray-200"}`}
              >
                📎 {content.filename || "File"} ({content.size ? formatSize(content.size) : "?"})
              </div>
            );
          }
          return null;
        })}

        <div className={`text-xs text-gray-300 mt-0.5 ${isSent ? "text-right mr-1" : "ml-1"}`}>
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
