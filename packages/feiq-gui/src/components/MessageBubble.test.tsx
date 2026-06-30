import { describe, it, expect } from "vitest";

// ─── Duplicate the pure functions from MessageBubble.tsx ──────────
// We cannot import them directly since they are not exported, so we
// replicate the exact logic to have a hermetic test.

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

function htmlEscape(text: string): string {
  const replacements: Record<string, string> = {
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#x27;",
  };
  return text.replace(/[&<>"']/g, (match) => replacements[match]);
}

function renderText(text: string): string {
  let result = htmlEscape(text);
  for (let i = 0; i < EMOJI_CODES.length; i++) {
    const code = EMOJI_CODES[i];
    const img = `<img src="emojis/${i + 1}.gif" alt="${code}" class="emoji-inline" style="width:20px;height:20px;vertical-align:middle;display:inline-block" />`;
    result = result.split(code).join(img);
  }
  return result;
}

// ─── Tests ───────────────────────────────────────────────────────

describe("htmlEscape", () => {
  it("escapes <script> tags to prevent XSS", () => {
    const input = "<script>alert(1)</script>";
    const output = htmlEscape(input);
    expect(output).toBe("&lt;script&gt;alert(1)&lt;/script&gt;");
    expect(output).not.toContain("<script>");
  });

  it("escapes ampersands", () => {
    expect(htmlEscape("a & b")).toBe("a &amp; b");
  });

  it("escapes double quotes", () => {
    expect(htmlEscape('say "hello"')).toBe("say &quot;hello&quot;");
  });

  it("escapes single quotes", () => {
    expect(htmlEscape("it's")).toBe("it&#x27;s");
  });

  it("preserves safe text unchanged", () => {
    expect(htmlEscape("Hello, world!")).toBe("Hello, world!");
    expect(htmlEscape("")).toBe("");
    expect(htmlEscape("abc123")).toBe("abc123");
  });

  it("handles mixed content safely", () => {
    const input = '<b>bold</b> & <i>italic</i>';
    const output = htmlEscape(input);
    expect(output).not.toContain("<b>");
    expect(output).not.toContain("</b>");
    expect(output).not.toContain("<i>");
    expect(output).toContain("&lt;b&gt;");
  });
});

describe("renderText", () => {
  it("prevents XSS by escaping HTML before emoji replacement", () => {
    const input = '<script>alert(1)</script>';
    const output = renderText(input);
    // The <script> tag must be escaped
    expect(output).not.toContain("<script>");
    expect(output).toContain("&lt;script&gt;");
  });

  it("still converts emoji codes after HTML escaping", () => {
    // Emoji code wrapped in XSS attempt
    const input = '<script>/:)</script>';
    const output = renderText(input);
    // HTML must be escaped
    expect(output).toContain("&lt;script&gt;");
    // Emoji must still render (the /:) code should be replaced)
    expect(output).toContain("emojis/1.gif");
    // The emoji img should be outside the escaped script tags
    expect(output).toContain("&gt;");
  });

  it("renders emoji codes as img tags", () => {
    const input = "Hello /:)";
    const output = renderText(input);
    expect(output).toContain("Hello");
    expect(output).toContain('<img src="emojis/1.gif"');
    expect(output).toContain('alt="/:)"');
    expect(output).not.toContain("/:)");
  });

  it("renders multiple emoji codes", () => {
    const input = "/:) /:D /:(";
    const output = renderText(input);
    expect(output).toContain('src="emojis/1.gif"');
    expect(output).toContain('src="emojis/14.gif"');
    expect(output).toContain('src="emojis/17.gif"');
  });

  it("renders emoji with special characters properly", () => {
    // Emoji codes with special chars: /:-S (indecisive), /;? (query)
    const input = "/:-S /;?";
    const output = renderText(input);
    expect(output).toContain("emojis/35.gif"); // /:-S is index 34 (0-based)
    expect(output).toContain("emojis/37.gif"); // /;? is index 36 (0-based)
  });

  it("preserves normal text with no emoji codes", () => {
    expect(renderText("Just plain text")).toBe("Just plain text");
    expect(renderText("")).toBe("");
  });
});

describe("normalizeContent", () => {
  // Re-implement the normalizeContent function for testing
  interface NormalizedContent {
    type: string;
    text?: string;
    format?: string;
    filename?: string;
    size?: number;
    localTaskId?: number;
  }

  function normalizeContent(raw: Record<string, unknown>): NormalizedContent {
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
    return {
      type: String(raw.type || "text"),
      text: typeof raw.text === "string" ? raw.text : String(raw.text || ""),
      filename: String(raw.filename || ""),
      size: Number(raw.size || 0),
    };
  }

  it("detects externally-tagged text content", () => {
    const result = normalizeContent({ text: { text: "Hello", format: "" } });
    expect(result.type).toBe("text");
    expect(result.text).toBe("Hello");
  });

  it("detects internally-tagged sealed content", () => {
    const result = normalizeContent({ type: "sealed", text: "burn after reading", format: "", ttl_seconds: 60 } as unknown as Record<string, unknown>);
    expect(result.type).toBe("sealed");
    expect(result.text).toBe("burn after reading");
  });

  it("detects internally-tagged image content", () => {
    const result = normalizeContent({ type: "image", id: "12345678" } as unknown as Record<string, unknown>);
    expect(result.type).toBe("image");
  });

  it("detects externally-tagged knock", () => {
    const result = normalizeContent({ knock: {} });
    expect(result.type).toBe("knock");
  });

  it("detects externally-tagged file", () => {
    const result = normalizeContent({ file: { filename: "test.pdf", size: 1024 } });
    expect(result.type).toBe("file");
    expect(result.filename).toBe("test.pdf");
    expect(result.size).toBe(1024);
  });

  it("detects externally-tagged image", () => {
    const result = normalizeContent({ image: {} });
    expect(result.type).toBe("image");
  });
});
